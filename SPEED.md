Speeding up model responses in a retrieval‑augmented setup like this involves looking at both the model itself and everything around it (ingestion, retrieval, transport, and the runtime environment). Here are the main levers you can pull:

### 1. Tighten the retrieval pipeline
- **Limit retrieved chunks**: In `backend/src/retriever.rs` (or whichever retrieval stage feeds the LLM), reduce `top_k` / rerank depth so the downstream model has less context to process.
- **Use faster embeddings**: If chunk embedding is happening on the request path, consider a lighter model (e.g., MiniLM, bge-small, etc.) or precompute embeddings asynchronously so queries only run a vector similarity search.
- **Cache popular queries**: Use the L1/L2 caches and optional Redis L3 already baked in. Make sure `REDIS_ENABLED=true` and TTLs are tuned so repeated questions reuse previous answers.

### 2. Optimize the model itself
- **Quantize**: If you’re loading the model locally (via the `llm` crate, for example), go for 4‑bit or 8‑bit quantized weights to reduce compute time.
- **Smaller checkpoints**: Evaluate whether you need the current model size for your accuracy requirements; stepping down from a 13B to a 7B or 3B model can cut latency dramatically.
- **Specialized models**: For short factual answers, distilled models (like `distilbert`, `mistral-instruct`, etc.) often respond faster without much quality loss.

### 3. Improve concurrency and batching
- **Batch requests**: Group multiple user requests internally if you serve many simultaneous users; modern inference stacks (like `tch`, `tensorrt_llm`, or vLLM) support micro-batching.
- **Async background work**: Keep chunking, indexing, and expensive I/O off the hot path. If you haven’t already, ensure `SKIP_INITIAL_INDEXING=false` only during initial ingestion, and move maintenance tasks to `reindex_async`.

### 4. Hardware and runtime tuning
- **GPU vs CPU**: If you’re still CPU-bound, moving the inference step to a GPU (or even a better CPU with AVX512) can be a big win.
- **Pin threads**: Make sure `TOKIO_WORKER_THREADS` (or equivalent) is configured appropriately so long-lived inference tasks don’t starve Actix’s thread pool.
- **Use a serving engine**: Instead of running the model inside your Actix process, offload to a dedicated inference server (like TGI, vLLM, or text-generation-webui) and call it over HTTP/gRPC; these are optimized for throughput and latency.

### 5. Measure, then iterate
- **Tracing + metrics**: Use the existing `/monitoring/metrics` and OTEL traces to identify whether the bottleneck is retrieval, model inference, or post-processing. Look at histograms like `SEARCH_HISTO_BUCKETS` and add custom spans around inference to get real numbers.
- **Profile**: If inference is local Rust code, run `perf`, `tokio-console`, or flamegraphs to confirm which functions consume the most time.

Start by measuring where the latency sits (retrieval vs model vs response streaming), then apply the targeted tweaks above. Let me know which part seems slowest and I can help dive deeper into that code path.
