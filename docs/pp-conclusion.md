# pp4 Phase 2 Step 1 — fragmentation distribution, post-PDF-fix

## Final results — post-PDF-fix (n=13)

| Q | Query | sections | docs | sec_ratio | doc_ratio | gap |
|---|-------|----------|------|-----------|-----------|-----|
| 1 | What is systemd? | 7 | 1 | 0.875 | 0.125 | **0.750** |
| 2 | Hitachi's revenue | 5 | 3 | 0.625 | 0.375 | 0.250 |
| 3 | Who is Pieter? | 3 | 3 | 0.375 | 0.375 | 0 |
| 4 | Sony Corp | 8 | 2 | 1.000 | 0.250 | **0.750** |
| 5 | Reconciler | 8 | 1 | 1.000 | 0.125 | **0.875** |
| 6 | systemd user services | 8 | 3 | 1.000 | 0.375 | **0.625** |
| 7 | Hitachi segments | 7 | 4 | 0.875 | 0.500 | 0.375 |
| 8 | Sony vs SIE | 7 | 1 | 0.875 | 0.125 | **0.750** |
| 9 | Mitsubishi | 7 | 1 | 0.875 | 0.125 | **0.750** |
| 10 | EXI themes | 7 | 7 | 0.875 | 0.875 | 0 |
| 11 | systemd unit types | 6 | 3 | 0.750 | 0.375 | 0.375 |
| 12 | Compare 3 conglomerates | 6 | 1 | 0.750 | 0.125 | **0.625** |
| 13 | hammer 1–5 diff | 8 | 2 | 1.000 | 0.250 | **0.750** |

## Compared to pre-fix (where gap was 0 on all 13 queries)

| metric | pre-fix | post-fix |
|---|---|---|
| Avg gap | 0.000 | **0.535** |
| Max gap | 0 | 0.875 |
| Queries with gap > 0 | 0/13 | 11/13 |
| Queries with gap ≥ 0.5 | 0/13 | 8/13 |

## What changed between the two runs

The fragmentation signal itself was unchanged. The corpus pipeline was
fixed:

1. **`flat_text_ir` → `pdf_paged_ir`** (`backend/src/index.rs`). PDFs no
   longer collapse to a single `Text` block; `pdftotext`'s form-feed page
   markers (`\x0c`) now drive `PageBreak` block emission, so the chunker
   creates one `section_id` per page.
2. **`DocIR::push` admits strong-boundary blocks** (`backend/src/doc_ir.rs`).
   The original push silently dropped any block whose `text` was empty
   unless it was atomic — and `PageBreak` is empty by design. Strong-
   boundary blocks now bypass the empty-text filter.
3. **`pdf_paged_ir` splits BEFORE preprocessing**. `extract_text_from_bytes`
   runs `apply_text_preprocessing` and the normalizer, both of which treat
   `\x0c` as whitespace and strip it. `pdf_paged_ir` now calls
   `extract_text_from_pdf_pdftotext` directly, splits on `\x0c`, and only
   then applies the per-page preprocessing.
4. **`doc_id` field is `STRING | STORED`** (`backend/src/retriever.rs`).
   The compound chunk-id (`"{filename}#{i}"`) was tokenized at index time,
   which made `delete_term` a silent no-op for the Tantivy queryability
   layer. The comment ("STRING (not TEXT)…") had documented the intent
   but the code disagreed.

Together those produce the signal pp4 was originally aiming at:
within-document fragmentation, observable independently of cross-document
spread.

## Threshold call

The gap distribution suggests a natural cut around **0.5**:

- queries with `gap ≥ 0.5` (8/13) look like the "PointerRag would help"
  case — the chunks are real fragments of one or two documents spread
  across many sections;
- queries with `gap < 0.5` (5/13) look like coherent or genuinely
  cross-doc retrieval — Strict or Hybrid is fine.

Pp4 Phase 2 Step 1 is now actionable. Picking **`section_ratio - doc_ratio ≥ 0.5`** as the auto-trigger threshold is defensible on this corpus.
That decision is reversible — the threshold should be re-validated when
the corpus shape changes (more diverse PDFs, more long-form markdown).

## The threshold, conceptually

### What it is

A single number that splits Auto-mode queries into two routing buckets:

```
gap = section_ratio - doc_ratio

if gap ≥ threshold  →  PointerRag (hydrate full sections)
else                →  fall through to today's Strict/Hybrid decision
```

`gap` is the *within-document* fragmentation signal. `section_ratio` says
how spread the matches are across sections; `doc_ratio` says how spread
they are across documents. The **difference** isolates the part of the
spread that's happening *inside* documents — which is exactly what
PointerRag was built to address.

### What it represents

The threshold is the system's answer to: **"how scattered does retrieval
inside a document have to be before reassembling the full sections beats
handing the LLM the raw chunks?"**

- A low threshold (say 0.2) — fires PointerRag aggressively. Most multi-
  section queries get full-section hydration. Risk: spending Pointer's
  section-fetch cost on queries that didn't need it.
- A high threshold (say 0.8) — fires PointerRag only when retrieval is
  *extremely* scattered. Risk: missing cases where Pointer would have
  helped.
- A threshold equal to 0 — Auto becomes PointerRag-first.
- A threshold equal to 1 (impossible to exceed) — Auto never picks
  PointerRag.

So the threshold is a knob that lives on a continuum from "always
Pointer" to "never Pointer," with the meaningful tuning happening in the
middle.

### Why it matters

Three reasons, in order of how durable they are:

**1. It's the system's heuristic for when Pointer's hypothesis applies.**
Pointer's whole premise is "the matched chunks are fragments of a larger
coherent section that the LLM needs to see whole." That premise is true
sometimes and false other times — the threshold encodes how confident
the system needs to be before acting on it.

**2. It converts an observability signal into a routing decision.**
Without it, the fragmentation gap is just a number in the log. With it,
the gap becomes a behavior — the user feels it (the chat answer is
different) without having to read step traces. That's the "make the
invisible visible *and actionable*" step.

**3. It's a forcing function for corpus knowledge.** Any picked
threshold is implicitly a claim about what shapes of fragmentation occur
in this corpus. When the corpus changes — bigger PDFs, more headered
markdown, mixed-language docs — the same threshold may stop being right,
and the system will quietly under- or over-route. That's why the
threshold isn't a constant *forever*; it's the first answer that gets
revisited as data accumulates.

### Why getting it exactly right matters less than it sounds

The threshold is **a starting point, not a verdict**. Three reasons it
can be sloppy and still work:

- It only routes *Auto* mode. Users who explicitly pick PointerRag bypass
  it; users who pick Strict bypass it. The threshold only decides for
  users who haven't decided themselves.
- A wrong decision is observable. The step trace shows the gap that
  triggered the route and the resulting `hydrated/fallback` counters. If
  Auto routes badly, you can see why and adjust.
- It's a single mutable number, not architecture. Changing 0.5 → 0.6 is
  a one-line edit (or a runtime setting). The architectural commitment
  is the *signal* (which already shipped in Phase 1); the threshold is
  the cheap follow-up.

### The danger to watch

The threshold's main failure mode is **calcification**: someone picks a
number that worked on the corpus-at-decision-time, then the corpus
drifts, and the threshold quietly under-serves users for months because
nobody re-runs the data analysis. Two defenses:

- **Log the gap alongside the route taken**, so threshold misbehavior is
  visible in any future trace audit (the Phase 1 instrumentation already
  does this).
- **Pick the threshold with a known-rough sample**, document why it was
  picked, and frame it as provisional in the followups. Then the next
  person who looks at it has the context to challenge it instead of
  assuming it was carefully chosen.

That's why pp4 was careful to make Phase 1 (instrument) and Phase 2
(decide) separate steps — the threshold is the *fragile* commitment;
the signal is the *durable* one.

## Exposing the threshold as a user-tunable setting

The threshold above is framed as a single number to ship in code, but
it fits ag's existing runtime-overrides pattern better as a knob the
user can tune themselves. The case:

### For

- **ag already does this for analogous knobs.**
  `RECONCILER_AUTO_MERGE_THRESHOLD` is the exact parallel — a heuristic
  cutoff baked into the runtime overrides system, surfaced in
  `/config/runtime`. Adding `POINTERRAG_AUTO_GAP_THRESHOLD` (or similar)
  lands cleanly in the existing 27-key pattern.
- **The threshold is fundamentally corpus-dependent.** Whatever value
  is picked from 13 queries on this corpus has no reason to be right for
  another user's corpus. The person who can tune it defensibly is
  whoever owns the corpus — i.e. the user, not us.
- **It's already a low-blast-radius knob.** `section_id` is per-page,
  the gap is computed from observable data, the route is only Auto-mode.
  Users can tweak it without fear of breaking anything; worst case is
  some Auto queries take a slightly different path.
- **Educational fit.** ag is framed as a learning platform — "make the
  invisible visible." Exposing the knob with an info modal that explains
  *what gap means* and *what 0.5 means* is exactly the design pattern
  CLAUDE.md describes. The user learns *by* tuning.
- **Defense against calcification.** A knob that's expected to be tuned
  doesn't rot the same way a constant does. The instinct shifts from
  "what's the right value?" to "what's the right value *here*?"

### Against (worth surfacing)

- **Most users won't tune it.** They'll leave the default. So picking a
  sensible default still matters — the 13-query analysis isn't wasted;
  it's the default-picking exercise.
- **Three thresholds is more than one.** `high_confidence` already has
  `chunks ≥ 3 AND tokens ≥ 1536` baked in. Adding `gap ≥ X` makes the
  Auto routing logic visibly more complex. That's a documentation cost,
  not a correctness one, but it's real.
- **Without comparison telemetry, "is 0.6 better than 0.5?" is
  unanswerable for a casual tuner.** The user can move the knob but
  can't easily know if their tweak helped. Mitigations exist (the
  step-trace already shows the gap), but they're not as good as a
  side-by-side A/B.

### Recommendation

Ship it as a runtime override with default 0.5. Two specific design
choices:

1. **Default lives in code as a `const`**, override reads through
   `settings::effective_or("POINTERRAG_AUTO_GAP_THRESHOLD", 0.5)`. Same
   pattern as the `effective_bool` / `effective_or` calls used
   elsewhere. Hot-reloadable.
2. **Info modal on the config page** explains what the gap is, what the
   threshold does, and points at the step-trace surface so users can
   self-calibrate. Same shape as the Reconciler info modal.

That converts pp4 Phase 2 Step 1 from "pick a threshold" into
"pick a default + expose the knob" — which is the version of Step 1
that fits ag.

## Effect of changing the threshold value

The threshold gates the Auto→Pointer route. Concretely, against the
13-query distribution above:

### Walking through values

| threshold | queries routed to Pointer | which ones | behavior |
|-----------|---------------------------|------------|----------|
| 0.0 | 13/13 | all | Auto becomes "always Pointer" — every query reassembles sections, even genuinely coherent retrievals |
| 0.25 | 11/13 | all except gap=0 (Pieter, EXI themes) | Fires on anything with within-doc spread at all |
| **0.5** | **8/13** | **Q1, Q4, Q5, Q6, Q8, Q9, Q12, Q13** | **Recommended — splits the bimodal gap cleanly** |
| 0.7 | 6/13 | only gap ≥ 0.75 | Conservative — Pointer only when retrieval is heavily scattered |
| 0.9 | 1/13 | only Q5 (reconciler, gap=0.875) | Pointer ~never — effectively disables the auto-route |
| 1.0 | 0/13 | none | Pointer never auto-fires; Auto reverts to today's Strict/Hybrid logic |

### What the user feels at each end

- **Lower threshold (more aggressive):**
  - Per-query latency goes up — each Pointer query does N extra
    `fetch_section` reads
  - LLM context gets longer — full sections are bigger than raw
    chunks, so more tokens per call
  - Answer quality improves on genuinely fragmented retrievals;
    potentially degrades on coherent ones (the LLM gets more "noise"
    around the actual answer)
  - Cost goes up if using a paid LLM (more context tokens)

- **Higher threshold (more conservative):**
  - Behavior approaches today's Auto — Strict for high-confidence,
    Hybrid for low
  - Cheaper, faster
  - Misses the Pointer-wheelhouse cases (the gap=0.625+ queries that
    benefited from full-section reassembly)
  - The "fragmentation: section X, doc Y" step trace still surfaces —
    user sees the gap was X but routing didn't fire, so they have
    feedback to lower the slider

### Implicit interaction with the existing `high_confidence` check

The threshold runs *before* `high_confidence` per pp4's design. So
lowering the threshold doesn't just bias Pointer vs Hybrid — it also
bypasses Strict for queries that would have qualified. A low threshold
= "I'd rather have Pointer's hydrated context than Strict's grounded-
from-chunks answer." A high threshold = "Trust the existing
`high_confidence` routing."

### Practical sweet spot on this corpus

Anything in **0.45–0.55** picks the same 8/13 queries (the natural
cluster boundary). Anything in **0.625–0.75** picks 6/13 (drops Q6 and
Q12 which are within-doc-fragmented but only across 6 sections instead
of 7–8). The slider doesn't need granularity finer than step=0.05 —
most decisions happen in those two bands.

## Effect on RAG quality / answer quality

The threshold trade-off is **not linear**. Lower-Pointer isn't strictly
better, and neither is higher. There's a U-shaped quality curve that
bends depending on the query and the document shape.

### What changes for the LLM's job at each end

The LLM's input changes shape based on the route:

- **Strict/Hybrid (high threshold)**: LLM sees N raw chunks, each ~400
  tokens, possibly mid-sentence at boundaries
- **Pointer (low threshold)**: LLM sees M full sections, each
  potentially 5–50× longer than a single chunk

That input-shape change is where quality is won or lost.

### Quality gains from lower thresholds (more Pointer)

| dimension | why Pointer helps |
|---|---|
| **Completeness** | When the chunk boundary cut mid-sentence or mid-paragraph, the surrounding section restores the missing context |
| **Grounding** | LLM sees the original prose flow, less likely to fabricate transitions or attribute things wrongly |
| **Tables / lists** | A chunk often contains a fragment of a table — the section restores the header row and other rows for context |
| **"I don't know" reduction** | Q5-style queries where the answer was in the next paragraph after the matched chunk |

### Quality losses from lower thresholds (more Pointer)

| dimension | why Pointer hurts |
|---|---|
| **Specificity** | Hydrated section is larger than necessary — LLM might lift a different sentence from the section than the one most relevant to the query |
| **Lost-in-the-middle** | Models reliably attend less to content in the middle of long context. A short, targeted chunk often outperforms a long, well-hydrated section for narrow questions |
| **Drift / tangents** | Long sections include surrounding discussion that may share vocabulary with the query but not the actual answer — LLM can wander |
| **Cross-section contradictions** | Multiple hydrated sections might lightly contradict each other (e.g., different revisions of the same fact) — LLM has to reconcile and may pick wrong |

### Per-query-type, very rough

- **Factoid lookups** (Q1, Q3, Q4, Q5): Lower threshold tends to help
  when chunks miss surrounding definitions, **hurts** when the answer
  was already in the chunk and Pointer just adds noise.
- **Synthesis / compare** (Q6, Q12): Lower threshold reliably helps —
  these queries benefit from broader context per source.
- **List / enumeration** (Q11 "systemd unit types"): Lower threshold
  helps if list spans pages, hurts if the list was already in one chunk
  and Pointer adds unrelated content.
- **Specific entity** (Q4 "Sony Corp", Q8 "Sony vs SIE"): Lower
  threshold helps when entity context is split across pages; can hurt
  if the corpus has multiple entities with similar names and Pointer
  pulls all their sections.

### The hallucination angle

Hallucinations come from two opposite causes:

- **Insufficient context** (high threshold, chunks miss the answer) →
  LLM fills gaps by inventing
- **Misleading context** (low threshold, hydrated section discusses
  adjacent-but-wrong content) → LLM confidently quotes the wrong thing

Both routes produce wrong answers; they fail differently. With
instrumentation, the user can tell which they're hitting:

- High threshold + frequent "I don't know" answers → context is too
  tight, lower the slider
- Low threshold + answers cite something almost-but-not-quite-right →
  context is too loose, raise the slider

### Practical advice for tuning

Start at 0.5 (the bimodal-split default). After running real queries:

- If answers feel **incomplete or cut off** → try 0.35
- If answers feel **drifty or off-topic** → try 0.65
- If you're seeing **"I don't know"** a lot → lower
- If you're seeing **answers that touch on the right area but miss the
  specific fact** → raise

The slider isn't optimizing a single number — it's choosing which
failure mode the user finds more tolerable. That's why exposing it as
a knob (rather than picking a "right" value once) is the design that
actually fits the variability.

## Why a single threshold is necessarily a compromise

A single static threshold *is* always a compromise, because the "right"
amount of Pointer depends on what kind of answer the user needs —
which the threshold doesn't know about.

### What "optimal" looks like

The ideal would be a per-query threshold:

```
threshold(query) = f(query_intent, retrieval_shape, doc_structure)
```

where the routing decision uses additional signals beyond just the
gap. A few that would be cheap to add:

- **Query type heuristics** — does the query contain "compare", "list",
  "summarize" (synthesis indicators) → bias toward Pointer? Does it
  look like a factoid lookup ("what is", "when did") → bias toward
  Strict?
- **Retrieval shape beyond the gap** — score spread across top-k (wide
  spread = weaker coherence → Pointer might help reassemble). The pp4
  followups doc actually listed this as a candidate signal.
- **Section size** — if the candidate sections are all huge (>5K
  tokens), the "lost-in-the-middle" risk dominates → bias toward
  Strict regardless of gap.
- **Doc structure signal** — heavily-headered markdown vs flat PDFs vs
  uniform corpora behave very differently. The system could observe
  its own retrieval distribution and weight accordingly.

### Why we don't do that yet

Each of those signals has cost:

- **Query classification** adds either an LLM call (latency + cost) or
  a brittle regex (false positives). Both produce a *probabilistic*
  signal that needs its own threshold… and now you've moved the
  calibration problem one level up.
- **Score spread** is free to compute but requires deciding what
  "wide" means — another threshold.
- **Section size check** is cheap and useful, but the right cutoff
  depends on the LLM's context window, which varies per-user.
- **Doc structure adaptation** requires tracking corpus statistics
  over time, which is a whole observability subsystem.

So the slope from "one threshold" → "smart router" is steep, and each
step adds a new calibration that itself benefits from a knob. You can
recurse forever.

### The practical compromise

A single user-tunable threshold has one nice property the smart router
doesn't: **the user can develop intuition for it**. After 20 queries,
the user knows whether 0.5 is too aggressive or too conservative *for
their typical workload*. A multi-signal router obscures that — answers
vary for reasons the user can't easily map to a single dial.

### Staged path forward

1. **Today:** ship the slider, default 0.5. User learns their
   workload-appropriate value. (Pp4 Phase 2.)
2. **Next:** add the *cheapest* refinement that would help — probably
   "max section size cap" so Pointer doesn't fire when the section
   would blow the context window. That's a second knob, but a cheap-
   to-understand one.
3. **Later:** if step 2 isn't enough, add score-spread as a tie-breaker
   on the borderline cases (gap close to threshold). One step closer
   to "adaptive."
4. **Eventually:** when there's enough usage data, train query-
   classification on the actual query → answer-quality pairs that ag
   has accumulated. That's the real "smart router," but it needs
   ground truth that doesn't exist yet.

Each step is testable and reversible. None of them require committing
to "the right answer" — they each add another observable knob.

### Honest summary

The ideal threshold *does* depend on query type and corpus shape, and
the system that adapts to that doesn't exist yet. The threshold slider
is the compromise that lets the user find a per-workload-average
optimum without committing the system to predicting their queries.
Approximating the ideal is a multi-PR journey, and pp4 explicitly
bounds itself at the first step.

## How to decide *when* to add the next refinement

The staged path above says step 3 is "add score-spread as a tie-breaker
on borderline cases." But how do you know step 2 isn't enough? Three
answers, increasing in honesty.

### The naive answer

Add it when you have a hunch. Bad path — leads to over-engineering
without evidence.

### The metric-driven answer

Add it when you can show that score-spread *correlates with quality*
on borderline cases. Concretely:

1. Run with the slider at default (0.5). Log per-query: `gap`,
   `route_chosen` (Pointer vs Strict/Hybrid), `top_k_score_distribution`,
   and ideally a quality signal (thumbs up/down, or human eval).
2. Filter to *borderline* queries: those with `0.4 ≤ gap ≤ 0.6`
   (within ±0.1 of the threshold).
3. In that subset, check: does `score_spread` (the variance or range
   of top-k chunk scores) predict which of those borderline queries
   was answered better by Pointer vs Strict?
4. If yes → score-spread is a useful tie-breaker, add it. If no →
   it's noise, save the engineering effort.

Concretely, you'd be looking for something like: "borderline-gap
queries with HIGH score-spread were answered better by Pointer; LOW
score-spread → Strict was better." If that pattern is statistically
clear (not necessarily significant — even directional with n=20 is
enough at this stage), the signal is real.

### The behavioral answer

Add it when the *user's slider behavior* tells you to. Two patterns
that would trigger this:

- **Slider volatility.** If users frequently move the slider mid-
  session ("lower for this query, raise for this one"), the single
  threshold isn't enough — they're mentally classifying queries the
  system isn't. That's the cue for a second signal.
- **Sliders converge to extremes.** If users keep moving to 0.1 or
  0.9 in practice, they're saying "binary, not continuous" — Pointer-
  mostly or never. That's a *different* signal need: maybe per-query-
  type opt-in, not a tie-breaker.

### What you should actually do

Don't decide in advance. Ship the slider. Add this to the followups doc:

> Decision point for adding a tie-breaker:
>
> 1. Wait until there are ≥50 logged Auto-mode queries with gap data
> 2. Manually review the 10–20 in the borderline band (gap 0.4–0.6)
> 3. For each, judge whether Strict or Pointer answered better
> 4. Check if any *other* signal (score spread, query length, doc size)
>    correlates with that judgment
> 5. If yes → add it as a tie-breaker. If no → leave it.

That's the empirical path: instrumentation is already there from Phase
1, the decision is "wait for enough data and then look." You'd know
"step 2 isn't enough" because step 2 wouldn't fix the visible
borderline failures — they'd persist in the trace.

### The recursive trap to avoid

Don't add the tie-breaker because it *sounds* like a good idea. Each
layer added "in advance" calcifies — it commits the system to a
heuristic before the data justifies it, and the next person can't tell
which heuristics are load-bearing and which are vestigial. The bar for
adding routing complexity should be: **a visible failure mode the
current system can't explain or fix with its existing knobs**.

## Can the decision loop itself be automated?

The 5-step decision recipe above could itself be automated. The
automation shape has natural staging that mirrors what ag could
realistically build.

### What each step looks like automated

| step | manual | automated form |
|---|---|---|
| 1. Wait for ≥50 queries | watch a counter | trivial — count logged queries with gap data, fire a notification at threshold |
| 2. Identify borderline queries | filter the log | trivial — query for `gap ∈ [0.4, 0.6]` |
| 3. Judge Strict vs Pointer per query | human eval | **the hard part** — needs a quality signal that doesn't exist today |
| 4. Check correlation with other signals | spreadsheet | straightforward — Pearson/Spearman correlation between candidate signal and quality score |
| 5. Decide whether to add it | judgment | surfaced as a recommendation, not auto-applied |

Step 3 is where automation actually gets expensive. Three sub-options
for the quality signal:

**Option A — LLM-as-judge.** Run both routes (Strict + Pointer) for
each borderline query. Send both answers + the user's question to a
stronger model and ask "which is better and why?"
- Pros: scalable, semi-objective, doesn't need user attention
- Cons: doubles LLM cost per borderline query; introduces a second
  LLM's bias as ground truth; the judge model may itself be wrong
  systematically
- Realistic cost: ~$0.05/query at GPT-4-ish prices × 50 borderline
  queries = $2.50. Cheap if it's a one-off study; expensive if it's
  continuous.

**Option B — Shadow routing + delayed user feedback.** Don't pick a
route — run both, send the active one's answer to the user, but cache
the inactive answer. Track whether the user re-queries shortly after
(implicit "answer was bad" signal). Correlate the recovered answer
with the bad-signal.
- Pros: uses real user behavior as ground truth
- Cons: re-query is a noisy signal; the cached inactive answer is
  never validated; doubles LLM cost per borderline query
- Doable in ag's existing observability layer, but the "user re-queried
  within N minutes" signal is brittle.

**Option C — Explicit user feedback (thumbs up/down).** Already
standard pattern. Cheapest infrastructure, but depends on user actually
clicking.
- Pros: real signal, low cost
- Cons: sparse — most users won't bother. Sample of 50 borderline
  queries might yield 5 with feedback.

### The realistic staged automation path

The full pipeline would be:

```
[gap-instrumented queries]
    ↓
[auto-aggregate borderline subset]
    ↓
[quality scoring via A/B/C]
    ↓
[correlation analysis: which signal predicts quality?]
    ↓
[surface recommendation to operator: "score_spread shows r=0.42, consider adding it"]
    ↓
[human approves → code change → deploy]
```

What ag could realistically build *now*, sorted by effort:

1. **Cheapest (a week).** Step 1–2 + 4 automated, leaving step 3
   (quality judgment) manual via a UI: "here are 10 borderline queries
   from the past week; click better/worse on each pair." User does
   the eval, system does the correlation. Closes the loop fast without
   building LLM-as-judge.

2. **Medium (a month).** Add LLM-as-judge (option A) for shadow
   routing. Auto-runs nightly, produces a weekly report: "this week's
   borderline subset showed score_spread correlation r=X with judge-
   preferred answers." Operator reads the report and decides.

3. **Most ambitious (quarter+).** Full closed loop: system auto-
   proposes signal additions, A/B tests them in production behind a
   flag, auto-promotes if quality improves. This is the territory of
   "ML routing" and it's a real project — needs a feature-flag system,
   A/B framework, statistical significance gates, rollback logic.

### The honest take on whether to automate this

The decision to automate is itself a meta-decision that benefits from
the same empirical discipline:

> **Don't automate this until you've manually done it once.**

Run the loop once by hand (≥50 queries, manually judge 10, correlate
signals in a notebook). If you find the manual process *was* useful
and the data *did* point at a clear signal, *then* automating it is
justified by demonstrated value. If you manually do it and find no
clear signal, automating wouldn't have helped either — it would have
produced the same null result with more engineering cost.

Same recursive trap as before: don't build infrastructure for a
problem you don't yet know exists.
