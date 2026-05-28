#!/usr/bin/env bash
# Sweep all atoms: prune auto-source tags, retry_tagging, log result.
#
# Run AFTER scripts/retag-dryrun.sh confirms the new tagging behavior is
# correct. Requires `tagging_strategy=truncated_full_content` in the active
# registry; otherwise KNN inheritance will paint over the clean LLM output.
#
# Per-atom flow:
#   1. snapshot tags (TSV)
#   2. DELETE FROM atom_tags WHERE atom_id=? AND source='auto'
#   3. POST /api/tagging/retry/{atom_id}
#   4. poll tagging_status until != processing/pending
#   5. log before/after counts
#
# Manual rows are preserved. Snapshots saved for restore.

set -euo pipefail

DB="${ATOMIC_DB:-/Users/brandonkiefer/Library/Application Support/com.atomic.app/databases/default.db}"
HOST="${ATOMIC_HOST:-http://localhost:44380}"
TOKEN="${ATOMIC_TOKEN:-$(cat "/Users/brandonkiefer/Library/Application Support/com.atomic.app/local_server_token")}"

SNAP_DIR="/tmp/retag-sweep-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$SNAP_DIR"

# Verify tagging_strategy is LLM-only — KNN would re-pollute every atom.
strategy=$(sqlite3 "/Users/brandonkiefer/Library/Application Support/com.atomic.app/registry.db" \
  "SELECT value FROM settings WHERE key='tagging_strategy'")
if [[ "$strategy" != "truncated_full_content" ]]; then
  echo "ERROR: tagging_strategy is '$strategy', not 'truncated_full_content'."
  echo "Set it first: curl -X PUT -H \"Authorization: Bearer \$TOKEN\" -H 'Content-Type: application/json' \\"
  echo "  -d '{\"value\":\"truncated_full_content\"}' \"\$HOST/api/settings/tagging_strategy\""
  exit 1
fi

ATOM_IDS=$(sqlite3 "$DB" "SELECT id FROM atoms ORDER BY created_at ASC")
total=$(echo "$ATOM_IDS" | wc -l | tr -d ' ')
echo "Sweeping $total atoms; snapshots → $SNAP_DIR"
echo

# Pre-sweep stats
echo "Pre-sweep totals:"
sqlite3 "$DB" \
  "SELECT printf('  total atoms: %d', COUNT(*)) FROM atoms;
   SELECT printf('  total atom_tags rows: %d', COUNT(*)) FROM atom_tags;
   SELECT printf('  rows per atom (avg): %.1f', (SELECT COUNT(*) FROM atom_tags) * 1.0 / NULLIF((SELECT COUNT(*) FROM atoms), 0));"
echo
echo "Top 10 tags pre-sweep (by direct atom count):"
sqlite3 -separator '   ' "$DB" \
  "SELECT printf('  %4d  %s', COUNT(*), t.name)
     FROM atom_tags at JOIN tags t ON t.id = at.tag_id
    GROUP BY at.tag_id ORDER BY COUNT(*) DESC LIMIT 10"
echo

i=0
fail=0
start_ts=$(date +%s)
while IFS= read -r atom_id; do
  i=$((i+1))
  before_count=$(sqlite3 "$DB" "SELECT COUNT(*) FROM atom_tags WHERE atom_id='$atom_id'")

  # Snapshot for potential restore
  sqlite3 -separator $'\t' "$DB" \
    "SELECT atom_id, tag_id, source FROM atom_tags WHERE atom_id='$atom_id'" \
    > "$SNAP_DIR/$atom_id.tsv"

  sqlite3 "$DB" "DELETE FROM atom_tags WHERE atom_id='$atom_id' AND source='auto'"

  http_status=$(curl -sS -o /dev/null -w '%{http_code}' -X POST \
    -H "Authorization: Bearer $TOKEN" "$HOST/api/tagging/retry/$atom_id" || echo "curl_err")
  if [[ "$http_status" != "200" ]]; then
    echo "[$i/$total] $atom_id  retry_tagging http_status=$http_status — restoring snapshot"
    while IFS=$'\t' read -r aid tid src; do
      sqlite3 "$DB" "INSERT OR IGNORE INTO atom_tags (atom_id,tag_id,source) VALUES ('$aid','$tid','$src')"
    done < "$SNAP_DIR/$atom_id.tsv"
    fail=$((fail+1))
    continue
  fi

  # Poll tagging_status (5s budget per atom is plenty for truncated_full_content)
  tries=0
  while (( tries < 30 )); do
    s=$(sqlite3 "$DB" "SELECT tagging_status FROM atoms WHERE id='$atom_id'")
    [[ "$s" != "processing" && "$s" != "pending" ]] && break
    sleep 0.5
    tries=$((tries+1))
  done

  after_count=$(sqlite3 "$DB" "SELECT COUNT(*) FROM atom_tags WHERE atom_id='$atom_id'")
  printf '[%3d/%d] %s  %2d → %2d  status=%s\n' "$i" "$total" "$atom_id" "$before_count" "$after_count" "$s"
done <<< "$ATOM_IDS"

elapsed=$(( $(date +%s) - start_ts ))
echo
echo "Sweep complete in ${elapsed}s. Failures: $fail"
echo
echo "Post-sweep totals:"
sqlite3 "$DB" \
  "SELECT printf('  total atoms: %d', COUNT(*)) FROM atoms;
   SELECT printf('  total atom_tags rows: %d', COUNT(*)) FROM atom_tags;
   SELECT printf('  rows per atom (avg): %.1f', (SELECT COUNT(*) FROM atom_tags) * 1.0 / NULLIF((SELECT COUNT(*) FROM atoms), 0));"
echo
echo "Top 10 tags post-sweep:"
sqlite3 "$DB" \
  "SELECT printf('  %4d  %s', COUNT(*), t.name)
     FROM atom_tags at JOIN tags t ON t.id = at.tag_id
    GROUP BY at.tag_id ORDER BY COUNT(*) DESC LIMIT 10"
echo
echo "Snapshots saved at $SNAP_DIR (TSV per atom; restore via INSERT OR IGNORE)."
