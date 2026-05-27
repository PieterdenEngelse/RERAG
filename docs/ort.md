# ort parameters — what's tunable, what's wrapped, what's exposed

Companion to `docs/ort-onnx.md`. Survey of every knob the `ort` crate
(ONNX Runtime, version 2.0.0-rc.12) gives us, sorted by whether ag
already wraps it and whether it flows from outside the binary.

## 1. Already wrapped in `OnnxConfig`

About 25 knobs are wired through `backend/src/perf/onnx_embedder.rs`:

- **Threading**: `num_threads`, `inter_op_num_threads`,
  `allow_inter_op_spinning`, `allow_intra_op_spinning`,
  `independent_thread_pool`
- **Optimization**: `optimization_level` (Disable / Basic / Extended /
  All), `enable_aot_inlining`, `disabled_optimizers`,
  `optimized_model_path`, `approximate_gelu`
- **Memory**: `enable_mem_pattern`, `enable_cpu_mem_arena`,
  `use_env_allocators`, `use_device_allocator_for_initializers`,
  `use_prepacking`
- **Quantization**: `enable_quant_qdq`, `enable_double_qdq_remover`,
  `enable_qdq_cleanup`
- **Execution**: `execution_mode` (Sequential / Parallel),
  `deterministic_compute`, `no_env_execution_providers`
- **Profiling / logging**: `enable_profiling`,
  `profiling_output_path`, `log_id`, `log_level`, `log_verbosity`
- **ag-level (not strictly ort)**: `embedding_batch_size`,
  `normalize_output`, `pooling`, `allow_simple_tokenizer`

## 2. Wrapped but **not** flowing from env / runtime settings

Only **three** of those ~25 are reachable from outside the binary
today (see `backend/src/embedder.rs:185`):

| Knob | Env / setting |
|------|---------------|
| `model_path` | `ONNX_MODEL_PATH` |
| `embedding_dim` | derived from `EMBEDDING_MODEL` |
| `allow_simple_tokenizer` | `ONNX_ALLOW_SIMPLE_TOKENIZER` (Phase 8) |

Everything else uses `..Default::default()`. To make e.g.
`num_threads`, `optimization_level`, `enable_mem_pattern`, `pooling`,
or `normalize_output` operator-tunable, each needs a
`settings::effective_*("ONNX_...", default)` read in that struct
literal and a row on the **Config → Runtime** page. Mechanical work
— the wiring scaffold already exists.

## 3. ort features ag hasn't wrapped at all

- **Execution providers.** CPU is implicit (no EP registered). ort
  can plug CUDA, CoreML, TensorRT, DirectML, OpenVINO, ROCm, QNN,
  WebGPU — each is a builder method behind a Cargo feature. Would
  need the matching `ort` feature in `Cargo.toml`, an EP selector +
  per-EP options struct on `OnnxConfig`. Big lift, only useful when
  leaving CPU.
- **IoBinding.** Pre-allocates input/output buffers and binds them
  once, avoiding alloc per `embed()` call. Real win for fixed-batch
  inference. Not exposed.
- **`RunOptions` per call** — `tag`, `log_severity_level`,
  `terminate` signal. Useful for cancellable long-running embeds;
  would need to thread a handle through `embed()`.
- **`WorkloadType`** (`Default` vs `Efficient`) — hints the EP about
  prioritization, mostly relevant on heterogeneous hardware.
- **Custom op libraries** — registering ops outside the ONNX standard
  set. ag doesn't need this.
- **Free-text session config K/V**
  (`SessionBuilder::with_config_entry`) — escape hatch for any ort
  feature without a typed wrapper.

## Recommendation — cheapest high-value next move

Plumb the existing wrapped knobs through `settings::effective_*` so
the UI can experiment without rebuilds. Prime candidates because they
actually affect throughput / quality and are safe to flip live:

| Key | Default | Why it matters |
|-----|---------|----------------|
| `ONNX_NUM_THREADS` | 4 | Pin to physical core count; big on multi-core hosts |
| `ONNX_INTER_OP_NUM_THREADS` | 1 | Only matters with `execution_mode=parallel` |
| `ONNX_OPTIMIZATION_LEVEL` | all | Drop to `extended` if startup time matters; `disable` for debugging |
| `ONNX_ENABLE_MEM_PATTERN` | true | First request after restart can be slow with this on; flip off for short-lived processes |
| `ONNX_EMBEDDING_BATCH_SIZE` | 32 | Already partially live via the existing `set_embedding_batch_size`; align names |
| `ONNX_NORMALIZE_OUTPUT` | true | Only meaningful for HNSW DotProduct == cosine; rare to flip but useful for diagnosis |
| `ONNX_POOLING` | cls | The single biggest correctness lever for BGE / MiniLM models |

Each would add: a settings read in `embedder.rs:185`, a row on the
**Config → Runtime** page (with appropriate hot-reload vs
restart-required classification), and an info modal explaining what
the knob does (ag is a learning platform — see `CLAUDE.md`).
