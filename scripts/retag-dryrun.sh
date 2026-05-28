#!/usr/bin/env bash
# Dry-run retagging of N hand-picked atoms with the new tagging logic.
#
# For each atom: snapshot its current tags, prune all auto-source assignments
# (preserving manual + wiki-backed semantics handled by the server side), call
# /api/tagging/retry/{id}, poll until tagging_status leaves "processing", and
# print a side-by-side before/after of the tag set.
#
# Manual rows are preserved. Restoring an atom is `update_atom` with the
# captured tag_ids (printed at the end of each diff).

set -euo pipefail

DB="${ATOMIC_DB:-/Users/brandonkiefer/Library/Application Support/com.atomic.app/databases/default.db}"
HOST="${ATOMIC_HOST:-http://localhost:44380}"
TOKEN="${ATOMIC_TOKEN:-$(cat "/Users/brandonkiefer/Library/Application Support/com.atomic.app/local_server_token")}"

ATOMS=(
  "ae2452b6-1b94-4c6b-b43e-03b39ead8431|MISTAG|Meeting Transcript Processor — System Prompt"
  "71bfcd2f-af4a-4550-8c2b-d004299b588f|MISTAG|Agent Profile: Brandon Kiefer"
  "4da7ad4e-4f50-4547-a6c9-f55a1f412cf6|MISTAG|Brainstorm — SQL Ops Repo"
  "b35ab231-73de-486f-8b32-3a3db93699aa|REAL  |newholder_payment — \$2M Collection Bypass Investigation"
  "a41d0d0a-e530-4c40-b0a5-11b5e3d2520b|REAL  |Case 00239912 — DFA ABC/ATCB Online Renewals"
)

snapshot_tags() {
  local atom_id="$1"
  sqlite3 -separator $'\t' "$DB" \
    "SELECT at.tag_id, t.name, at.source
       FROM atom_tags at
       JOIN tags t ON t.id = at.tag_id
      WHERE at.atom_id = '$atom_id'
      ORDER BY at.source DESC, t.name ASC"
}

current_tag_names() {
  local atom_id="$1"
  sqlite3 "$DB" \
    "SELECT t.name
       FROM atom_tags at
       JOIN tags t ON t.id = at.tag_id
      WHERE at.atom_id = '$atom_id'
      ORDER BY t.name ASC" | paste -sd ',' -
}

tagging_status() {
  local atom_id="$1"
  sqlite3 "$DB" "SELECT tagging_status FROM atoms WHERE id = '$atom_id'"
}

prune_auto_tags() {
  local atom_id="$1"
  # Aggressive prune for dry-run: drop every auto-source row so the new
  # tagging logic starts from a clean slate. Manual rows stay. Wiki articles
  # themselves stay (live in wiki_articles, not atom_tags).
  sqlite3 "$DB" "DELETE FROM atom_tags WHERE atom_id = '$atom_id' AND source = 'auto'"
}

retry_tagging() {
  local atom_id="$1"
  curl -sS -X POST -H "Authorization: Bearer $TOKEN" \
    "$HOST/api/tagging/retry/$atom_id" | head -c 200
  echo
}

wait_for_tagging() {
  local atom_id="$1"
  local tries=0
  while (( tries < 60 )); do
    local s
    s=$(tagging_status "$atom_id")
    if [[ "$s" != "processing" && "$s" != "pending" ]]; then
      echo "  → tagging_status=$s"
      return 0
    fi
    sleep 1
    tries=$((tries+1))
  done
  echo "  → timeout waiting; status=$(tagging_status "$atom_id")"
}

restore_tags() {
  local atom_id="$1"
  shift
  # Snapshot is "tag_id<TAB>name<TAB>source\n" repeated. Restore by re-inserting.
  while IFS=$'\t' read -r tag_id _name source; do
    [[ -z "$tag_id" ]] && continue
    sqlite3 "$DB" \
      "INSERT OR IGNORE INTO atom_tags (atom_id, tag_id, source) VALUES ('$atom_id', '$tag_id', '$source')"
  done <<< "$1"
}

print_diff() {
  local label="$1" before="$2" after="$3"
  echo "  BEFORE ($(echo "$before" | wc -l | tr -d ' ') tags):"
  echo "$before" | awk -F'\t' '{ printf "    [%s] %s\n", $3, $2 }'
  echo "  AFTER  ($(echo "$after" | wc -l | tr -d ' ') tags):"
  echo "$after" | awk -F'\t' '{ printf "    [%s] %s\n", $3, $2 }'
  # Diff the tag-name sets
  local b_names a_names
  b_names=$(echo "$before" | awk -F'\t' '{print $2}' | sort)
  a_names=$(echo "$after"  | awk -F'\t' '{print $2}' | sort)
  echo "  DROPPED:"
  comm -23 <(echo "$b_names") <(echo "$a_names") | sed 's/^/    - /'
  echo "  ADDED:"
  comm -13 <(echo "$b_names") <(echo "$a_names") | sed 's/^/    + /'
}

mkdir -p /tmp/retag-dryrun
SNAP_DIR=/tmp/retag-dryrun/$(date +%Y%m%d-%H%M%S)
mkdir -p "$SNAP_DIR"
echo "Snapshots: $SNAP_DIR"
echo

for entry in "${ATOMS[@]}"; do
  IFS='|' read -r atom_id kind title <<< "$entry"
  echo "=========================================================================="
  echo "[$kind] $title"
  echo "  id=$atom_id"
  before=$(snapshot_tags "$atom_id")
  printf "%s\n" "$before" > "$SNAP_DIR/$atom_id.before.tsv"

  prune_auto_tags "$atom_id"
  echo "  pruned auto tags; remaining count after prune: $(sqlite3 "$DB" "SELECT COUNT(*) FROM atom_tags WHERE atom_id='$atom_id'")"

  retry_tagging "$atom_id"
  wait_for_tagging "$atom_id"

  after=$(snapshot_tags "$atom_id")
  printf "%s\n" "$after" > "$SNAP_DIR/$atom_id.after.tsv"
  print_diff "$title" "$before" "$after"
  echo
done

echo "=========================================================================="
echo "All snapshots saved under $SNAP_DIR"
echo "To restore an atom, INSERT OR IGNORE rows from <id>.before.tsv into atom_tags."
