# TODO: External Infrastructure Requirements

These 5 optimizations require external infrastructure to implement:

| # | Optimization | What's Needed | Why |
|---|--------------|---------------|-----|
| **24** | **GPU Embeddings** | NVIDIA GPU + CUDA drivers | Embedding models run on GPU for 10-100x speedup. Requires `cudarc` or `candle` crate with CUDA backend |
| **26** | **ONNX Runtime** | ONNX Runtime library installed | Microsoft's optimized inference engine. Requires `ort` crate and ONNX Runtime C library |
| **27** | **Model Distillation** | Training infrastructure + dataset | Creating a smaller, faster model from a larger one. Requires ML training pipeline |
| **28** | **Batched GPU Inference** | NVIDIA GPU + CUDA drivers | Same as #24 - maximizing GPU utilization with batched requests |
| **31** | **Edge Caching** | CDN service (CloudFlare, Fastly, etc.) | Caching responses at edge locations worldwide. Requires CDN subscription |

---

## Implementation Notes

### 24 & 28: GPU Embeddings / Batched GPU Inference

```bash
# Install CUDA (Ubuntu)
sudo apt install nvidia-cuda-toolkit

# Add to Cargo.toml
candle-core = { version = "0.4", features = ["cuda"] }
# or
cudarc = "0.10"
```

### 26: ONNX Runtime

**What it is:** Microsoft's high-performance inference engine that runs optimized ML models.

**Benefits:**
- 2-10x faster inference than native PyTorch/TensorFlow
- Cross-platform (CPU, GPU, NPU)
- Optimized for production deployment

**Installation Steps:**

```bash
# 1. Download ONNX Runtime (Linux x64)
wget https://github.com/microsoft/onnxruntime/releases/download/v1.17.0/onnxruntime-linux-x64-1.17.0.tgz
tar -xzf onnxruntime-linux-x64-1.17.0.tgz
sudo mv onnxruntime-linux-x64-1.17.0 /opt/onnxruntime

# 2. Set environment variables
export ORT_DYLIB_PATH=/opt/onnxruntime/lib/libonnxruntime.so
export LD_LIBRARY_PATH=/opt/onnxruntime/lib:$LD_LIBRARY_PATH

# 3. Add to ~/.bashrc for persistence
echo 'export ORT_DYLIB_PATH=/opt/onnxruntime/lib/libonnxruntime.so' >> ~/.bashrc
echo 'export LD_LIBRARY_PATH=/opt/onnxruntime/lib:$LD_LIBRARY_PATH' >> ~/.bashrc

# 4. Add to Cargo.toml
# ort = "2.0"
```

**Convert embedding model to ONNX:**
```python
# Python script to convert HuggingFace model to ONNX
from transformers import AutoModel, AutoTokenizer
import torch

model_name = "sentence-transformers/all-MiniLM-L6-v2"
model = AutoModel.from_pretrained(model_name)
tokenizer = AutoTokenizer.from_pretrained(model_name)

# Export to ONNX
dummy_input = tokenizer("Hello world", return_tensors="pt")
torch.onnx.export(
    model,
    (dummy_input["input_ids"], dummy_input["attention_mask"]),
    "embedding_model.onnx",
    input_names=["input_ids", "attention_mask"],
    output_names=["embeddings"],
    dynamic_axes={
        "input_ids": {0: "batch", 1: "sequence"},
        "attention_mask": {0: "batch", 1: "sequence"},
        "embeddings": {0: "batch"}
    },
    opset_version=14
)
print("Model exported to embedding_model.onnx")
```

---

### 27: Model Distillation

**What it is:** Training a smaller, faster "student" model to mimic a larger "teacher" model.

**Benefits:**
- 2-10x smaller model size
- 2-5x faster inference
- Maintains 90-98% of original accuracy

**Requirements:**
1. Teacher model (e.g., `all-mpnet-base-v2` - 420MB)
2. Student architecture (e.g., `all-MiniLM-L6-v2` - 80MB)
3. Training dataset (domain-specific text corpus)
4. GPU for training (8GB+ VRAM recommended)

**Training Script (Python):**

```python
# distill_model.py
import torch
from sentence_transformers import SentenceTransformer, losses
from torch.utils.data import DataLoader

# 1. Load teacher and student models
teacher = SentenceTransformer('sentence-transformers/all-mpnet-base-v2')
student = SentenceTransformer('sentence-transformers/all-MiniLM-L6-v2')

# 2. Prepare training data (your domain-specific texts)
train_texts = [
    "Your domain-specific text 1",
    "Your domain-specific text 2",
    # ... thousands of examples
]

# 3. Create distillation dataset
from sentence_transformers import InputExample
train_examples = [InputExample(texts=[text]) for text in train_texts]
train_dataloader = DataLoader(train_examples, batch_size=32, shuffle=True)

# 4. Define distillation loss
loss = losses.MSELoss(model=student)

# 5. Train with teacher supervision
student.fit(
    train_objectives=[(train_dataloader, loss)],
    epochs=3,
    warmup_steps=100,
    output_path='./distilled_model',
    teacher_model=teacher
)

print("Distilled model saved to ./distilled_model")
```

**Estimated Resources:**
- Training time: 2-8 hours on single GPU
- Dataset size: 10K-1M text samples
- GPU memory: 8-16GB VRAM

---

### 31: Edge Caching (CDN)

**What it is:** Caching API responses at edge locations worldwide for lower latency.

**Benefits:**
- 50-200ms latency reduction for global users
- Reduced server load (cache hits don't reach origin)
- DDoS protection included

**Option 1: CloudFlare (Recommended - Free Tier Available)**

```bash
# 1. Sign up at https://cloudflare.com
# 2. Add your domain and update nameservers
# 3. Enable caching rules in dashboard

# CloudFlare-specific cache headers in Rust:
.insert_header(("Cache-Control", "public, max-age=3600"))
.insert_header(("CDN-Cache-Control", "max-age=86400"))  // CloudFlare respects this
.insert_header(("CF-Cache-Status", "DYNAMIC"))  // For debugging
```

**Option 2: AWS CloudFront**

```bash
# 1. Create CloudFront distribution in AWS Console
# 2. Point origin to your API server
# 3. Configure cache behaviors:

# Cache policy for search results:
{
  "DefaultTTL": 300,
  "MaxTTL": 3600,
  "MinTTL": 60,
  "QueryStringBehavior": "whitelist",
  "QueryStrings": ["q", "top_k"]
}
```

**Option 3: Fastly**

```bash
# 1. Sign up at https://fastly.com
# 2. Create service and add backend
# 3. Configure VCL for caching:

sub vcl_fetch {
  if (req.url ~ "^/search") {
    set beresp.ttl = 5m;
    set beresp.grace = 1h;
  }
}
```

**Rust Code for Cache Headers:**

```rust
// Add to search endpoint response
HttpResponse::Ok()
    .insert_header(("Cache-Control", "public, max-age=300, stale-while-revalidate=60"))
    .insert_header(("Vary", "Accept-Encoding"))  // Important for compression
    .insert_header(("ETag", format!("\"{}\"", hash_of_results)))
    .json(results)
```

**Cache Invalidation:**

```bash
# CloudFlare API
curl -X POST "https://api.cloudflare.com/client/v4/zones/{zone_id}/purge_cache" \
  -H "Authorization: Bearer {api_token}" \
  -H "Content-Type: application/json" \
  --data '{"purge_everything":true}'

# AWS CloudFront
aws cloudfront create-invalidation --distribution-id {dist_id} --paths "/*"
```

**Estimated Costs:**
- CloudFlare: Free tier (100K requests/day), Pro $20/month
- AWS CloudFront: ~$0.085/GB transfer, $0.0075/10K requests
- Fastly: ~$0.12/GB transfer, $50/month minimum
