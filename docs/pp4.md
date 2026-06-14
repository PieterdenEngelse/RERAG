# Plan: Instrument fragmentation signal in Auto mode

## Context

The followups doc carries an entry for an auto-trigger that would route
`AgentMode::Auto` queries into PointerRag when matched chunks look
fragmented (`docs/proxy-pointer-rag-followups.md` →
"PointerRag auto-trigger from fragmentation signal").

That entry explicitly defers two decisions until corpus data exists:
1. **Threshold** — "needs measurement before picking a number"
2. **UX** — silent switch vs step-trace hint vs UI suggestion

This plan ships the **measurement step** only. It instruments the
fragmentation ratio inside `AgentMode::Auto`, surfaces it in logs and
in the step-trace message, and changes no routing. The threshold and UX
decisions land later, with real data to inform them.

Symmetrical to action #1: PointerRag now emits hydrated/fallback
counters; Auto would now emit a fragmentation ratio. Same observability
shape, same "make the invisible visible" learning-platform alignment.

## Cross-phase note

The free function introduced in Phase 1 step 1 is the same surface
Phase 2 step 3 routes on. If its signature is designed well now, Phase 2
doesn't refactor — it just adds a threshold parameter (or wraps the
function in a routing decision). Phase 1's unit tests then double as
Phase 2 scaffolding, not just regression armor.

Concretely: keep the helper a free function over `&[String]` (the chunk
contents) plus an injected `Fn(&str) -> Option<String>` callback for the
section-id lookup. The Auto arm calls it with a closure that delegates
to `Retriever::meta_for_content`. Tests pass in synthetic closures and
never touch Tantivy or the `Mutex`.

## Phase 1 — observe-only (3 steps)

### Step 1: Compute fragmentation metrics

Add a free function (`fragmentation(chunks: &[String], lookup: impl
Fn(&str) -> Option<String>) -> FragmentationStats`) returning `tracked`,
`untracked`, `unique_sections`, and `ratio: Option<f32>` (None when
`tracked == 0`).

Lock-failure handling: the Auto arm's closure returns `None` for every
chunk when `self.retriever.lock()` fails. The helper then reports
`tracked == 0` naturally — no special-case branch needed.

**Tests** (unit, in `agent.rs` `#[cfg(test)] mod tests`):
- All chunks share one section_id → `ratio = 1/N`, `untracked = 0`.
- Every chunk has a distinct section_id → `ratio = 1.0`, `untracked = 0`.
- Mixed: some chunks missing section_id → `untracked` counts them,
  `unique_sections` ignores them, `ratio = unique_sections / tracked`.
- Empty input → no panic; `tracked = 0`, `ratio = None` (not `NaN`).
- Lock-failure simulated via callback returning `None` for every chunk
  → all `untracked`, `tracked = 0`, no division by zero.

### Step 2: Emit `tracing::info!` with the breakdown once per Auto query

Fields: `chunks`, `tracked`, `untracked`, `unique_sections`, `ratio`
(or `"unknown"` when `None`), plus the existing `est_tokens`.

**Tests**:
- Capture log lines with `tracing::subscriber::with_default` plus a
  `tracing_subscriber::fmt::MakeWriter` writing into `Vec<u8>`.
- One test per Auto branch (high-confidence and low-confidence) asserts
  the log line contains the four field names and values from step 1.
- **Anti-test**: same query run twice produces exactly two log lines,
  not one or three. Catches accidental hoisting or omission.

### Step 3: Append fragmentation suffix to the step-trace message

Suffix format (locked once via test, not re-bikeshed): something like
`(fragmentation: 0.83, 5/6 chunks tracked)` for the known case and
`(fragmentation: unknown, 0/6 chunks tracked)` when the ratio is
undefined. Applies to both the high-confidence and low-confidence
`AgentStep` branches.

**Tests** (unit, against `AgentResponse.steps`):
- High-confidence Auto run → last `kind:"llm"` step message ends with
  the fragmentation suffix in the agreed format (assert exact substring
  via `assert_eq!` on the suffix — `insta` is overkill for one line).
- Low-confidence Auto run → same assertion.
- Suffix format includes all four numbers so a reader scanning the
  trace can reconstruct the ratio without re-querying logs.

## Phase 2 — decide and route (3 steps)

### Step 1: Pick threshold

No tests — data analysis, not code. Output is a number written into the
design entry and the eventual constant.

### Step 2: Pick UX (silent / step-trace hint / suggest-in-UI)

No tests — decision, not code. The tests come with whichever shape
ships in step 3.

### Step 3: Wire routing decision into Auto, before `high_confidence`

**Tests** (unit, using the same callback seam from Phase 1):
- Fragmented retrieval (ratio above threshold) → Auto routes to
  PointerRag regardless of `high_confidence`.
- Non-fragmented + high-confidence → `RagStrict` (existing behavior
  preserved).
- Non-fragmented + low-confidence → `Hybrid` (existing behavior
  preserved).
- **Boundary**: ratio exactly at the threshold → asserts the documented
  inclusive/exclusive convention. Pick one and lock it.
- All chunks untracked (older index, no `section_id`s) → does **not**
  trigger PointerRag (fragmentation only meaningful over `tracked > 0`);
  falls through to the `high_confidence` gate. Regression-prevention
  test for the mixed-vintage-index case PointerRag already flags.

**UX-dependent tests** (only one of these ships, matching step 2's
choice):
- Silent: assert step trace shows `PointerRag: hydrated …` message, no
  extra "switched from Auto" line.
- Step-trace hint: assert hint string appears in `steps` with the
  fragmentation numbers.
- Suggest-in-UI: backend test asserts a flag/field in `AgentResponse`
  (the surface the frontend reads); frontend test is its own follow-up.

## Out of scope (not deferred — wrong project for this PR)

- Touching `AgentMode::PointerRag` or any other mode. Auto only.
- Modal copy changes, info-button changes, frontend signal plumbing.
- Refactoring the existing `high_confidence` check or its constants.

## Deferred followups-entry fix

Three text replacements in `docs/proxy-pointer-rag-followups.md` queued
for later:
- `770-799` → `770-836`
- "section-id lookup" → "PointerRag arm"
- `781-783` → `781-784`

No tests — docs only. Verification is a single grep:
`grep -n "770-836\|PointerRag arm\|781-784" docs/proxy-pointer-rag-followups.md`
returns the expected hits.

## File to modify

- `backend/src/agent.rs` — the `AgentMode::Auto` arm inside
  `run_with_mode`, plus the new free helper near the top of the file
  and a `#[cfg(test)] mod tests` block (if one doesn't already exist).
  Two anchors:
  - Insert the fragmentation computation **before** the existing
    `let context = used_chunks.join("\n\n");` line (currently
    `agent.rs:742`), so the variables are in scope for both step-message
    branches and the `tracing::info` call.
  - Append the fragmentation suffix to the `format!` strings inside
    both `AgentStep` branches (currently `agent.rs:746-752` and
    `agent.rs:758-764`).

Single file. No new dependencies, no schema or config changes.

## Verification

Phase-level: every step's unit tests pass via `cargo test --lib agent::`
(or whatever path matches once the helper lands). The routing decision
in Auto is byte-identical to today for the same query — confirm by
comparing the chosen branch (`call_llm_strict` vs `call_llm`)
before/after on a fixed query. Only the step message text and
`tracing::info` line change.

`cargo fmt` + `cargo clippy --all-targets -- -D warnings` should pass;
existing tests should still pass.
