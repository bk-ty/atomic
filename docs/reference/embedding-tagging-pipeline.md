# Embedding and Auto-Tagging Pipeline

This document describes Atomic's current embedding and auto-tagging pipeline: how work is queued, how stages execute, how progress is reported, and where to extend the system.

The core implementation lives in `crates/atomic-core/src/embedding.rs`. The queue storage API lives in `crates/atomic-core/src/storage/traits.rs`, with SQLite and Postgres implementations in:

- `crates/atomic-core/src/storage/sqlite/chunks.rs`
- `crates/atomic-core/src/storage/postgres/chunks.rs`

The frontend progress UI is driven by WebSocket events normalized in `src/lib/transport/event-normalizer.ts` and consumed in `src/hooks/useEmbeddingEvents.ts`.

## Goals

- Batch provider calls efficiently regardless of job source.
- Keep auto-tagging after embedding so downstream tag effects can assume chunks and embeddings are current.
- Support embed+tag, embed-only, and tag-only work.
- Preserve current large-atom tagging behavior while keeping room for future chunk-assisted tagging.
- Keep the queue durable and safe under concurrent SQLite/Postgres workers.

## Core Data Model

Pipeline work is represented by one atom-level queue row in `atom_pipeline_jobs`.

Important fields:

- `atom_id`: the queued atom.
- `embed_requested`: whether the embedding stage should run.
- `tag_requested`: whether the tagging stage should run.
- `not_before`: earliest time the job can be claimed.
- `state`: `pending` or `processing`.
- `lease_until`: processing jobs become claimable again after the lease expires.
- `attempts`: claim count.
- `atom_updated_at`: snapshot from the atom row at enqueue time.

SQLite uses `atom_id` as the primary key. Postgres uses `(atom_id, db_id)`.

Enqueue is coalescing: multiple requests for the same atom merge stage flags with logical OR, reset the row to `pending`, clear any lease/error, and refresh `atom_updated_at`. This lets single-atom edits, imports, retries, re-embedding, startup recovery, and scheduled work all flow into the same queue.

## Job Sources

The main enqueue paths are exposed through `AtomicCore` in `crates/atomic-core/src/lib.rs`.

- `create_atom`: enqueue embed+tag.
- `create_atoms_bulk`: enqueue embed+tag for every inserted atom.
- `update_atom`: enqueue embed+tag.
- `process_atom_pipeline`: enqueue embed+tag for the latest persisted atom content.
- `process_pending_embeddings` / `process_pending_embeddings_due`: backfill queue rows from status columns.
- `process_pending_tagging` / `process_pending_tagging_due`: backfill queue rows from status columns.
- `reembed_all_atoms` / `spawn_reembed_pending`: enqueue embed-only.
- `retry_embedding`: enqueue embedding, and only request tagging if the atom already had `tagging_status = pending`.
- `retry_tagging`: enqueue tag-only and force `auto_tagging_enabled = true` for that run.

Agent write tools also enqueue through the same queue.

## Execution Flow

`process_queued_pipeline_jobs` claims due jobs in batches and starts one background queue run.

Within a run:

1. Claim jobs with a lease.
2. Emit `PipelineQueueStarted`.
3. Process all embedding-requested jobs with cross-atom chunk batching.
4. Mark successful embedded atoms complete and mark graph maintenance dirty.
5. Build the tagging set:
   - tag-only jobs are included immediately.
   - embed+tag jobs are included only if embedding succeeded.
   - embed-only jobs are excluded.
6. Emit tagging progress only after the tagging set is known.
7. Run tagging for the eligible atoms.
8. Clear processed queue rows.
9. Emit `PipelineQueueCompleted`.

This means tagging can never complete before chunks and embeddings are current for the atom. Semantic edges and tag centroids are deferred to graph maintenance so bulk imports and one-off edits use the same stable graph refresh path.

## Execution Modes

### Embed + Tag

Used for normal create/update/finalize flows.

The atom is chunked, embedded, saved, and marked dirty for graph maintenance. Tagging then runs if requested and enabled.

### Embed Only

Used when embeddings need to be regenerated without changing tags, such as an embedding model or dimension change.

For atoms with existing chunks, the pipeline reuses those chunk rows and only recalculates their embeddings. Atoms without chunks fall back to whole-atom chunking. Existing tags/tagging status are preserved.

Changing the embedding model queues embed-only work even when the new model has the same vector dimension. Equal dimensions do not mean equal vector spaces. If the dimension changes, storage clears old chunk vectors and recreates the vector index, but keeps chunk ids/content so the queue can still re-embed existing chunks instead of re-chunking every atom. The current chunk size assumes supported embedding models can handle roughly 1000-token chunks; a model-specific chunk budget can be added later if needed.

### Tag Only

Used when tagging should be retried without touching chunks or embeddings.

The job is claimable only when the atom already has `embedding_status = complete`, so tagging still depends on an embedded atom.

## Provider Batching

Embedding is chunk-based. The batch path groups atoms, chunks their content, and sends chunk texts to the embedding provider in adaptive batches.

Constants in `embedding.rs`:

- `EMBEDDING_BATCH_SIZE`: max texts per embedding provider call before adaptive splitting.
- `ATOM_FETCH_BATCH_SIZE`: max atom bodies fetched and chunked at once.
- `EMBEDDING_GROUP_CHUNK_TARGET`: target chunks per completion group. A single atom can exceed this limit so atom completion remains all-or-nothing.
- `PENDING_BATCH_SIZE`: max queue jobs claimed per DB claim.

If a provider rejects a batch with a retryable or batch-size-related error, the batch is split recursively until it succeeds or individual chunks fail.

Tagging is atom-based today. It runs concurrently under the LLM semaphore, but each atom is tagged independently. Large atoms are still handled by the existing truncation strategy in `extract_tags_from_content`.

## Graph Maintenance

Embedding completion no longer recomputes semantic edges inline. Instead it sets `edges_status = pending` for affected atoms and marks `task.graph_maintenance.*` state in the data DB.

`GraphMaintenanceTask` runs through the existing scheduled-task architecture. The server ticks scheduled tasks every 15 seconds across every database, with the existing per-`(task, db)` lock. The task cheaply no-ops when graph state is clean. When graph state is dirty, it runs when either:

- the durable pipeline queue for that DB has drained to zero, or
- dirty state has exceeded the max staleness window.

When it runs, it claims pending edge atoms, recomputes semantic edges in batches, recomputes tag centroids for tags attached to those atoms, rebuilds FTS, and invalidates the canvas cache. The default max staleness is 300 seconds and can be overridden with `task.graph_maintenance.max_staleness_seconds`.

## Strategy Hooks

Two settings-backed strategy enums make current behavior explicit and provide future extension points:

- `embedding_strategy`
  - `rechunk_whole_atom` (current behavior)
  - `incremental_dirty_chunks` (future hook; currently falls back with a warning)
- `tagging_strategy`
  - `truncated_full_content` (current behavior)
  - `chunk_assisted` (future hook; currently falls back with a warning)

These exist so future dirty-chunk embedding and chunk-assisted tagging can be introduced without changing enqueue call sites.

## Progress Events

Per-atom events still exist for local UI correctness:

- `EmbeddingComplete`
- `EmbeddingFailed`
- `TaggingComplete`
- `TaggingFailed`
- `TaggingSkipped`

Queue-level events drive global progress indicators:

- `PipelineQueueStarted { run_id, total_jobs, embedding_total }`
- `PipelineQueueProgress { run_id, stage, completed, total }`
- `PipelineQueueCompleted { run_id, total_jobs, failed_jobs }`

The frontend should use queue-level events as the source of truth for global progress. It should not infer pipeline totals from `AtomCreated` or `AtomUpdated`.

Important detail: tagging totals are not emitted at queue start. They are emitted only after embedding has determined which embed+tag atoms actually reached tagging. This avoids reporting fake tagging work for embed failures or embed-only jobs.

The older `BatchProgress` event still exists for legacy phase labels, but new UI should prefer queue-level events for totals and completion.

## Concurrency and Durability

The queue is designed to work under concurrent SQLite and Postgres implementations.

- Enqueue uses an upsert/coalescing pattern.
- Claim moves rows to `processing` and sets `lease_until`.
- Expired processing rows can be reclaimed.
- Postgres uses `FOR UPDATE SKIP LOCKED`.
- SQLite claim uses one atomic `UPDATE ... RETURNING` statement over due rows.

Queue rows are cleared after their requested stages reach terminal status for that run. Failed atom stages are terminal from the queue's perspective; the atom status/error columns preserve retryable state for user action or backfill.

## Status Columns Still Matter

The atom status columns remain the durable user-visible state:

- `embedding_status`
- `embedding_error`
- `tagging_status`
- `tagging_error`
- `edges_status`

The queue says what work is requested. The atom columns say what state the atom is in.

The backfill helpers enqueue from status columns for startup recovery, scheduled draft processing, and compatibility with flows that mark atoms pending before queueing.

## Tag Catalog Gating

Two mechanisms keep the auto-tagger working from a curated subset of the
tag tree.

### `is_autotag_target` (per-tag flag)

Defined on every row of `tags`. The flag gates inclusion of **top-level
parents** in the candidate tag tree the LLM sees.

- `is_autotag_target = 1` on a top-level parent (`parent_id IS NULL`)
  → the parent is shown to the LLM with up to 50 of its highest-atom-count
  children.
- `is_autotag_target = 0` on a top-level parent → the parent and all its
  children are excluded from the LLM tree, regardless of atom counts.
- The flag is **not** consulted on children: every child of an autotag
  target inherits eligibility. A child with `is_autotag_target = 0` is
  still loaded into the tree if it has atoms (or if it sits among the top
  50 by atom count under its parent).

The query lives in `crates/atomic-core/src/extraction.rs` in
`get_tag_tree_for_llm` (sqlite path) and
`crates/atomic-core/src/storage/postgres/tags.rs` in
`get_tag_tree_for_llm` (postgres path). Both filter
`parent_id IS NULL AND is_autotag_target = TRUE` at the top level.

The migration `005_autotag_target.sql` backfills `Topics, People,
Locations, Organizations, Events` to TRUE on existing databases. New
top-level tags created via `set_tag_autotag_target` (admin UI) flip the
flag explicitly. The 2026-05-28 audit confirmed live data is
operator-correct: the user has chosen `Processes`, `Services`,
`Knowledge` as autotag targets and intentionally excluded
`Infrastructure` and `People` from the LLM's view.

### Per-atom tagging cap (registry settings)

Three registry settings (defaults shown) bound how many tags the LLM may
return for a single atom. All three are enforced by `atomic-core` at the
caller layer regardless of any user-supplied `tagging_prompt` override —
a runaway prompt cannot blow past the configured cap.

| Setting | Type | Default | Effect |
|---|---|---|---|
| `tagging_max_tags` | int | 5 | Schema `maxItems` and the "Maximum N tags" rule in the system prompt. |
| `tagging_prefer_existing` | bool | `true` | Gates the "Prefer existing tags shown below over inventing new ones" rule. |
| `tagging_max_new_tags` | int | 1 | Post-LLM filter: tags whose name is not already in the catalog (case-insensitive) are partitioned and truncated to this cap. Existing-name tags are never dropped. |

Settings are re-resolved per atom in `process_tagging_atom_inner`, so a
runtime change takes effect on the next ingest with no restart.

`tagging_max_new_tags = 0` is a "tree-locked" mode: the LLM may not
introduce any new names; only tags already in the catalog can be applied.
Recommended for production once the catalog is fully descriptive (after
T35 + T44 sweeps).

### Recovery: clearing wiki-backed auto tags

`POST /api/tagging/retag-all` accepts an optional JSON body
`{"force_prune_wiki_backed": true}`. Default `false` preserves the
legacy behavior (wiki-backed `source='auto'` rows survive the prune).
`true` drops every `source='auto'` row regardless of wiki article. Use
the force-prune mode during a corpus cleanup; it is the supported
replacement for the manual SQL pattern in
`scripts/retag-sweep-all.sh`.

## Test Harness

The real-world-ish pipeline harness lives in:

- `crates/atomic-core/tests/pipeline_tests.rs`
- `crates/atomic-core/tests/support/mod.rs`

`MockAiServer` starts a local HTTP server that mimics enough of the OpenAI-compatible API for end-to-end provider calls:

- `/v1/embeddings`
- `/v1/chat/completions`

The tests use real request serialization, real provider code, real response parsing, real chunking, real storage, real deferred graph maintenance, and real tag extraction. Only the network peer is fake.

Run:

```bash
cargo test -p atomic-core --test pipeline_tests
```

SQLite tests run by default. Postgres variants require the `postgres` feature and `ATOMIC_TEST_DATABASE_URL`.

Current focused coverage includes:

- full create -> embed -> edges -> tag flow
- update lifecycle and old edge cleanup
- draft save then explicit finalize
- delete cascade for chunks/edges
- queue progress semantics
- embed-only re-embedding
- tag-only retry

## Future Work

- Replace whole-atom rechunking with dirty-chunk tracking for targeted updates.
- Add chunk-assisted tagging as a user-selectable strategy for large atoms.
- Add queue status snapshots to `/api/embeddings/status` so progress can recover across frontend reconnects.
- Remove legacy status-claim helpers after all callers are fully queue-native.
