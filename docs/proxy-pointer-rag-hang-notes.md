# Proxy-pointer RAG bundle — upload-hang post-mortem

Companion to commits c23ba40 and 732badf. The original "Known issue"
section in c23ba40 and the bisect findings in 732badf described an
upload hang that turned out to be **state-dependent, not bundle-causal**.

## TL;DR

- Hang is resolved. `POST /upload` works on this branch with a clean
  release build (`strip = true`, no debug info).
- Root cause: stale Tantivy segments left over from a pre-bundle
  schema. The bundle adds `section_id_field` to the schema; the
  schema-recover patch at `backend/src/retriever.rs:539-558` detects
  the `Index::open_or_create` `SchemaError`, wipes the on-disk index,
  and rebuilds. Once that wipe-and-rebuild had run, subsequent uploads
  worked normally.
- The "busy-spinning Future" diagnosis from the bisect doc was wrong.
  The gdb signature (hash mixing + vtable dispatch) is also what a
  Tantivy writer looks like when it's trying to reconcile segments
  whose term dictionaries don't match the live schema.

## Deploy note for first install of this bundle

**On first restart after pulling this bundle, ag will wipe and rebuild
any existing Tantivy index** (per corpus). The cause is the new
`section_id_field` (STRING+STORED), which invalidates the existing
schema. Users will see:

- A one-time re-index delay on first boot proportional to corpus size
  (a few seconds per 100 docs on the dev box).
- Index size on disk may differ after the rebuild — that's expected,
  not a bug.

No manual action is required. The schema-recover path is in
`backend/src/retriever.rs:539-558`. If a deployer wants to skip the
implicit wipe, they can delete `~/.local/share/ag/index/<corpus>/`
themselves before starting ag.

## Confirmation

Six sequential `POST :3011/upload` requests on commit 732badf,
stripped release build, `RECONCILER_ENABLED=false`:

| File              | Wall time |
|-------------------|-----------|
| reconciler_test.md (cold) | 1.48 s |
| hammer_1.md       | 0.77 s |
| hammer_2.md       | 0.81 s |
| hammer_3.md       | 0.90 s |
| hammer_4.md       | 0.86 s |
| hammer_5.md       | 0.94 s |

All `status=200`, all chunks indexed via the `io_uring` writer path,
no thread above 25% CPU during or after the test.

## What didn't matter (audit trail, to save the next debugger time)

The bisect doc ruled out reconciler, chunker, and `index_chunk`
rewrite — those rulings stand, those weren't the bug either. The
debug-symbol toggle (`strip = false`, `debug = true`) was a red
herring; the hang doesn't return when the toggle is reverted.
