# Proxy-pointer RAG bundle — upload hang debug notes

Companion to commit c23ba40. Updates the "Known issue" section of that
commit's message with bisect findings from a second debug pass.

## TL;DR for whoever picks this up

Uploading any file to `POST :3011/upload` hangs the HTTP response
indefinitely. One `actix-rt|system` worker thread sits at 100% CPU in
userspace until process restart. Other workers respond instantly
(`/monitoring/health` returns in <20 ms). Tantivy indexing via the
file-watcher path completes cleanly; the hang is on the upload
handler's `index_prepared_doc` → `commit` → graph indexing chain, but
**no `graph::*` debug log fires** because the spin happens before
Phase 4.

## What's been ruled out

- **Reconciler**: set `RECONCILER_ENABLED=false`, hang persists.
- **Chunker (#1 breadcrumb / heading-stack)**: `chunker_factory.rs`
  fully reverted to `main`'s version (with `..ChunkMeta::default()`
  stubs to satisfy the new fields), hang persists.
- **`index_chunk` writer rewrite**: the bundle's non-batch path is
  functionally identical to `main`'s `add_document` — both create a
  fresh `IndexWriter` with 256 MB heap per call. Pre-existing pattern.
- **Pathological regex in `tools/entity_extractor.rs`**: file is
  unmodified by the bundle (working tree clean for `tools/`), and
  357 pre-bundle documents indexed through it without issue.
- **New `tokio::spawn` / explicit loop**: `git diff main..HEAD` shows
  none added.

## What's confirmed (gdb on stripped binary)

Hot thread = `actix-rt|system` (TID varies per restart). 22-deep stack,
all `?? ()` because release binary is stripped. Disassembly of frame
addresses (after PIE base adjustment):

- **Frame 0**: hash mixing — `rorx $0x2f`/`$0x2b`/`$0x20` + chained
  `xor` (signature of `AHasher` / `DefaultHasher::write`)
- **Frames 1–10**: repeated indirect calls via vtables (`call *0x18(%rax)`)
  interleaved with `cmp $0x4, <reg>` discriminant checks — the
  Tokio `Future::poll` machinery
- **Frames 14–21**: scheduler internals (`__rseq` thread-local
  updates, `futex`, `start_thread`)

This shape — vtable dispatch in a tight loop, hash op at the leaf —
matches a **busy-spinning Future**: one that returns `Poll::Pending`
while immediately re-waking itself (anti-pattern), so the scheduler
re-polls it forever without yielding to other tasks.

## Suspect surfaces, ranked

1. **Tantivy writer contention** in `Retriever::begin_batch()` or the
   commit at the end of `upload_search.rs:139`. File-watcher's
   `index_prepared_doc` log appears at the moment the upload arrives
   (the upload handler's Phase 1 saved the file, file_watcher noticed,
   indexed it via the non-batch path that creates its own writer). If
   the upload handler's `begin_batch()` then tries to acquire the
   global Tantivy writer lock while the file_watcher's writer is still
   draining, *and* this happens inside a Future that doesn't suspend
   correctly, the spin pattern fits.
2. **`add_mention` write path** added in `graph/knowledge_builder.rs` —
   even though dormant on the reconciler-off path, may have introduced
   a static-init or lazy-static that fires on first use.
3. **Field-dict initialization** for the new `section_id_field`
   (STRING+STORED) in Tantivy — first write to a new STRING field
   builds a fresh term dictionary; if the bundle's write path enters
   this initialization inside a non-yielding Future, same shape.

## How to reproduce (clean)

```bash
git switch feat/proxy-pointer-rag
cargo build --release --features graph
systemctl --user restart ag.service
# Make sure RECONCILER_ENABLED=false in overrides.json
curl -X POST -F file=@/tmp/anyfile.md -m 60 http://127.0.0.1:3011/upload
# → curl times out at 60 s; ag stays at 100% CPU on one actix-rt thread
# → file_watcher's Tantivy index entry appears in journalctl; nothing else
```

## How to confirm function names (cost: ~12 min)

```toml
# Cargo.toml [profile.release]
debug = true     # ← add this
strip = false    # ← flip this
```

Then `cargo build --release --features graph` and re-attach gdb:

```bash
PID=$(systemctl --user show ag.service -p MainPID --value)
gdb -p $PID -batch -ex "set pagination off" \
    -ex "thread apply all bt 30" -ex quit
```

Look for the thread with name `actix-rt|system` and CPU ticks orders
of magnitude above the others.

## Workaround for users

- `RECONCILER_ENABLED=false` (default) keeps existing graph behavior
- File-watcher ingest (drop files into `~/.local/share/ag/data/corpora/default/documents/`)
  works fine — that path doesn't go through `index_to_knowledge_graph`
- HTTP `/upload` is broken until the spin is fixed

## Patch I already added (independent of this hang)

`backend/src/retriever.rs:539-558` — auto-recover from Tantivy
SchemaError on `Index::open_or_create`. Wipes the on-disk index and
retries with the new schema. Necessary because the bundle's new
`section_id_field` invalidates any existing index. Keep this when
fixing the hang.
