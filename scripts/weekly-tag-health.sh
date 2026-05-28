#!/usr/bin/env bash
# scripts/weekly-tag-health.sh
#
# Run the tag-health report against a live Atomic server and emit a
# markdown summary on stdout. Intended for cron, manual review, or as a
# Slack/email payload.
#
# Required env vars (CLI-overridable):
#   ATOMIC_BASE_URL   default: http://127.0.0.1:44380
#   ATOMIC_TOKEN      default: read from the Tauri local_server_token file
#                              ($HOME/Library/Application Support/com.atomic.app/local_server_token)
#
# Usage:
#   scripts/weekly-tag-health.sh
#   scripts/weekly-tag-health.sh --base-url https://atomic.example.com --token "$MY_TOKEN"
#   scripts/weekly-tag-health.sh --json   # emit raw JSON instead of markdown

set -euo pipefail

BASE_URL="${ATOMIC_BASE_URL:-http://127.0.0.1:44380}"
TOKEN="${ATOMIC_TOKEN:-}"
EMIT_JSON=0

while [ $# -gt 0 ]; do
  case "$1" in
    --base-url) BASE_URL="$2"; shift 2 ;;
    --token)    TOKEN="$2"; shift 2 ;;
    --json)     EMIT_JSON=1; shift ;;
    -h|--help)
      sed -n '2,18p' "$0"
      exit 0
      ;;
    *) echo "Unknown arg: $1" >&2; exit 2 ;;
  esac
done

if [ -z "$TOKEN" ]; then
  TOKEN_FILE="$HOME/Library/Application Support/com.atomic.app/local_server_token"
  if [ -r "$TOKEN_FILE" ]; then
    TOKEN="$(cat "$TOKEN_FILE")"
  fi
fi

if [ -z "$TOKEN" ]; then
  echo "ERROR: no token. Pass --token, set ATOMIC_TOKEN, or place the token in" >&2
  echo "       \$HOME/Library/Application Support/com.atomic.app/local_server_token." >&2
  exit 2
fi

URL="${BASE_URL%/}/api/tagging/health-report"
RESPONSE="$(curl -fsS -H "Authorization: Bearer $TOKEN" "$URL")"

if [ "$EMIT_JSON" -eq 1 ]; then
  printf '%s\n' "$RESPONSE"
  exit 0
fi

# Markdown summary using jq for shape transformation.
if ! command -v jq >/dev/null 2>&1; then
  echo "ERROR: jq is required for markdown output. Install it or pass --json." >&2
  exit 3
fi

printf '%s\n' "$RESPONSE" | jq -r '
def pct(x): if x == null then "—" else "\((x*100*100|floor)/100)%" end;

"# Tag Health — \(.computed_at)",
"",
"**Atoms:** \(.total_atoms) (tagged: \(.tagged_atoms))   |   **Tags:** \(.total_tags)   |   **Assignments:** \(.total_tag_assignments)",
"",
"| Metric | Value |",
"| --- | --- |",
"| avg_tags_per_atom | \(.avg_tags_per_atom) |",
"| median_tags_per_atom | \(.median_tags_per_atom) |",
"| p95_tags_per_atom | \(.p95_tags_per_atom) |",
"| max_tags_per_atom | \(.max_tags_per_atom) |",
"| top_tag_rate | \(pct(.top_tag_rate)) |",
"| single_child_subtrees | \(.single_child_subtrees) |",
"| junk_drawer_tags (≥50%) | \(.junk_drawer_tags | length) |",
"| 100%-overlap pairs | \(.hundred_pct_overlap_pairs | length) |",
"| low_visibility_tags | \(.low_visibility_tags_count) |",
"| never_used_tags | \(.never_used_tags_count) |",
"",
(if (.regressions | length) == 0 then
  "## Regressions\n\nNone — corpus is within thresholds."
else
  "## Regressions\n\n" +
  ([.regressions[] |
    "- **\(.severity | ascii_upcase) — \(.check):** \(.message)"
  ] | join("\n"))
end),
"",
"## Top Tags",
"",
"| Tag | Atoms | Rate |",
"| --- | --: | --: |",
(.top_tags[] |
  "| \(.name) | \(.atom_count) | \(pct(.rate)) |"
),
"",
(if (.junk_drawer_tags | length) > 0 then
  "## Junk-Drawer Parents (≥50% direct)\n\n" +
  "| Tag | Direct | Subtree | Ratio |\n" +
  "| --- | --: | --: | --: |\n" +
  ([.junk_drawer_tags[] |
    "| \(.tag_name) | \(.direct_atoms) | \(.subtree_atoms) | \((.ratio * 1000 | floor)/10)% |"
  ] | join("\n"))
else "" end),
"",
(if (.single_child_examples | length) > 0 then
  "## Single-Child Subtrees\n\n" +
  ([.single_child_examples[] |
    "- `\(.parent_name)` → only child `\(.only_child_name)`"
  ] | join("\n"))
else "" end),
"",
(if (.hundred_pct_overlap_pairs | length) > 0 then
  "## 100%-Overlap Pairs (top \(.thresholds.overlap_top_n))\n\n" +
  ([.hundred_pct_overlap_pairs[] |
    "- **\(.tag_a_name)** ↔ **\(.tag_b_name)** — \(.atom_count) atoms"
  ] | join("\n"))
else "" end)
'
