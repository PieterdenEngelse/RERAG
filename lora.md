# LoRA for RAG: Requirements & Conditions

This document outlines the conditions that must be met to use LoRA (Low-Rank Adaptation) fine-tuning with your RAG system.

## Overview

LoRA allows you to fine-tune a base LLM on your specific domain data, teaching it to better utilize retrieved context and produce more accurate, grounded responses.

**Key Concept:** Train on cloud GPU (free Colab), deploy locally via Ollama.

---

## 1. Data Collection Requirements

| Requirement | Minimum | Recommended |
|-------------|---------|-------------|
| **Training examples** | 500+ | 1,000+ |
| **Quality score** | ≥3 (out of 5) | ≥4 |
| **Diversity** | Multiple query types | Cover all use cases |

### What Counts as a Training Example?

Each training example is a complete **instruction-response pair** that teaches the model how to answer questions using RAG context:

```json
{
  "instruction": "How do I configure rate limiting?",
  "context": "From docs: Rate limiting uses token buckets with RATE_LIMIT_TOKENS=100...",
  "response": "To configure rate limiting, set RATE_LIMIT_TOKENS in your .env file. The default is 100 tokens per minute per IP..."
}
```

### Sources for Training Examples

| Source | Description | How to Collect |
|--------|-------------|----------------|
| **User feedback** | Real Q&A interactions rated by users | Enable `TRAINING_DATA_ENABLED=true`, users rate responses |
| **Document chunks** | Your indexed documents converted to Q&A | Export via `/training/export_snapshot` |
| **Support conversations** | Historical support tickets/chats | Import and normalize to JSONL |
| **Synthetic generation** | LLM-generated Q&A from your docs | Use GPT-4 to generate questions about your content |

### What Makes a "Good" Example?

✅ **Good examples:**
- Question is realistic (something a user would actually ask)
- Context contains relevant retrieved chunks
- Response is grounded in the context (not hallucinated)
- Quality score ≥ 3

❌ **Bad examples:**
- Generic questions unrelated to your domain
- Missing or irrelevant context
- Response doesn't use the provided context
- Factually incorrect answers

### Why 500+ Minimum?

| Dataset Size | Result |
|--------------|--------|
| < 100 | Model barely learns anything |
| 100-300 | Overfitting risk, inconsistent results |
| **500-1000** | Minimum viable fine-tuning |
| 1000-5000 | Good generalization |
| 5000+ | Excellent domain adaptation |

The 500+ threshold is where LoRA fine-tuning starts producing meaningful improvements over the base model for domain-specific tasks.

### Enable Data Collection

```bash
# Add to .env or export
export TRAINING_DATA_ENABLED=true
export TRAINING_MIN_QUALITY=3
```

### Collect Feedback

As you use the RAG system, provide feedback on responses:

```bash
curl -X POST http://localhost:3010/training/feedback \
  -H "Content-Type: application/json" \
  -d '{
    "query": "What is Rust?",
    "response": "Rust is a systems programming language...",
    "context": "Retrieved document content...",
    "quality_score": 5,
    "mode": "hybrid"
  }'
```

### Check Progress

```bash
curl http://localhost:3010/training/stats
```

### Automating Training Data Generation

**TL;DR:** LoRA training is just teaching the model with Question + Context → Answer examples. This can be automated!

```
Q: "How do I enable rate limiting?"
+
Context: [relevant docs retrieved by RAG]
=
A: "Set RATE_LIMIT_TOKENS=100 in .env..."
```

#### Option 1: Automatic from User Interactions (Built-in)

Your system already collects training data automatically! When users rate responses:

```bash
# Enable in .env
TRAINING_DATA_ENABLED=true
TRAINING_MIN_QUALITY=3
```

Every thumbs up/down or quality score automatically saves a training example.

#### Option 2: Synthetic Q&A Generation

Use an LLM to automatically generate questions from your documents:

```python
# Pseudocode for synthetic data generation
for chunk in document_chunks:
    # Ask LLM to generate questions about this chunk
    questions = llm.generate(f"""
        Given this text, generate 3 realistic questions a user might ask:
        
        Text: {chunk.content}
        
        Questions:
    """)
    
    # For each question, generate an answer using RAG
    for question in questions:
        context = rag.retrieve(question)
        answer = llm.generate(question, context)
        
        save_training_example(question, context, answer)
```

#### Option 3: Batch Export + Auto-Generate (Hybrid)

```bash
# 1. Export your documents to JSONL
python tools/lora_training/scripts/export_docs.py

# 2. Run synthetic Q&A generation (uses Ollama + RAG API)
python tools/lora_training/scripts/generate_synthetic_qa.py \
  --questions-per-chunk 3 \
  --verbose

# Or with custom options:
python tools/lora_training/scripts/generate_synthetic_qa.py \
  --input tools/lora_training/data/docs_snapshot.jsonl \
  --output tools/lora_training/data/synthetic_qa.jsonl \
  --questions-per-chunk 5 \
  --ollama-model phi3.5:latest \
  --max-chunks 50
```

**Requirements for synthetic generation:**
- Ollama running (`ollama serve`)
- RAG backend running (`cd backend && cargo run`)
- Documents exported (`export_docs.py`)

#### Automation Summary

| Method | Effort | Quality | Speed |
|--------|--------|---------|-------|
| User feedback | Zero (automatic) | High (real usage) | Slow (depends on traffic) |
| Synthetic generation | Script setup | Medium (needs review) | Fast (batch process) |
| Hybrid | Medium | High | Medium |

**Recommendation:** Start with automatic user feedback collection. Supplement with synthetic generation if you need to reach 500+ examples faster.

### Synthetic Q&A Generation Script

The script `tools/lora_training/scripts/generate_synthetic_qa.py` automates training data creation:

**What it does:**
1. Reads document chunks from `docs_snapshot.jsonl`
2. Uses Ollama (your local LLM) to generate realistic questions
3. Retrieves context via your RAG API (`/search` endpoint)
4. Generates grounded answers using the retrieved context
5. Outputs training-ready JSONL with instruction/context/response format

**Basic usage:**
```bash
# First, export your documents
python tools/lora_training/scripts/export_docs.py

# Then generate Q&A pairs
python tools/lora_training/scripts/generate_synthetic_qa.py --verbose
```

**All options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--input` | `data/docs_snapshot.jsonl` | Input document file |
| `--output` | `data/synthetic_qa.jsonl` | Output Q&A file |
| `--questions-per-chunk` | 3 | Questions per document chunk |
| `--ollama-model` | `phi3.5:latest` | Model for generation |
| `--ollama-url` | `http://localhost:11434` | Ollama API URL |
| `--rag-url` | `http://localhost:3010` | RAG backend URL |
| `--max-chunks` | all | Limit documents to process |
| `--skip-existing` | off | Skip if output exists |
| `--verbose` | off | Show detailed progress |

**Example with all options:**
```bash
python tools/lora_training/scripts/generate_synthetic_qa.py \
  --input tools/lora_training/data/docs_snapshot.jsonl \
  --output tools/lora_training/data/synthetic_qa.jsonl \
  --questions-per-chunk 5 \
  --ollama-model phi3.5:latest \
  --max-chunks 100 \
  --verbose
```

**Prerequisites:**
- Ollama running: `ollama serve`
- RAG backend running: `cd backend && cargo run`
- Documents exported: `python tools/lora_training/scripts/export_docs.py`

**GUI Trigger:**

You can also trigger synthetic Q&A generation from the web UI:

1. Go to **Monitor → Index** page
2. Scroll to the **LoRA Export Controls** panel
3. Find the **Synthetic Q&A Generation** section (cyan border)
4. Configure:
   - **Questions per chunk**: 1-10 (default: 3)
   - **Max documents**: Leave blank for all, or set a limit
5. Click **Generate Q&A**

The status shows:
- Running/Idle state
- Number of examples generated
- Last run timestamp
- Any errors

6. Click **View Examples** to browse generated Q&A pairs
   - Paginated view (10 per page)
   - Shows question, answer, and expandable context
   - Source file reference for each example

**Output format:**
```json
{
  "instruction": "How do I configure rate limiting?",
  "context": "From docs: Rate limiting uses token buckets...",
  "response": "To configure rate limiting, set RATE_LIMIT_TOKENS in .env...",
  "source": "documents/config.md",
  "timestamp": "2025-02-17T10:30:00",
  "tags": ["synthetic", "auto-generated"],
  "metadata": {"generator": "generate_synthetic_qa.py", "model": "phi3.5:latest"}
}
```

---

## 2. Data Format Requirements

Each training example must include:

```json
{
  "instruction": "User's question",
  "context": "Retrieved RAG chunks (critical!)",
  "response": "Grounded answer using the context",
  "source": "documents/guide.md",
  "tags": ["monitoring", "neo4j"]
}
```

**Key insight:** Including RAG context in training teaches the model how to use retrieved information effectively. Without context, the model won't learn the retrieval-augmented pattern.

---

## 3. Hardware Requirements

| Phase | Hardware | Notes |
|-------|----------|-------|
| **Training** | GPU (NVIDIA T4+) | Use free Google Colab |
| **Deployment** | 8GB RAM minimum | Runs on CPU via Ollama |

### Training (Cloud)
- Google Colab provides free T4 GPU access
- Training typically takes 15-60 minutes depending on dataset size
- No local GPU required

### Deployment (Local)
- Ollama handles inference on CPU
- Quantized GGUF models fit in 4-8GB RAM
- No GPU required for inference

---

## 4. Software Requirements

### For Training (Cloud/Colab)

- Python 3.10+
- Unsloth library (optimized fine-tuning)
- PyTorch with CUDA
- PEFT (Parameter-Efficient Fine-Tuning)
- Transformers library

### For Deployment (Local)

- Ollama installed and running
- GGUF model file exported from training
- Modelfile for Ollama import

---

## 5. Model Selection Constraints

For 8GB RAM deployment, choose appropriately sized base models:

| Model | Parameters | GGUF Size | RAM Usage | Recommendation |
|-------|------------|-----------|-----------|----------------|
| **Phi-3.5-mini** | 3.8B | ~2.2GB | ~4GB | ✅ Best choice |
| Llama-3.2-3B | 3B | ~1.8GB | ~3.5GB | ✅ Good balance |
| Llama-3.2-1B | 1B | ~0.6GB | ~2GB | ✅ Fastest |
| Gemma-2-2B | 2B | ~1.2GB | ~3GB | ✅ 8K context only |
| Llama-3-8B | 8B | ~4.5GB | ~8GB | ⚠️ Tight fit |

### Quantization Levels

| Quantization | Quality | Size | Use Case |
|--------------|---------|------|----------|
| Q4_0 | Good | Smallest | Memory constrained |
| Q5_K_M | Better | Medium | Balanced |
| Q8_0 | Best | Larger | Quality priority |

---

## 6. Training Configuration

Scale LoRA parameters with your dataset size:

```python
# Small datasets (500-1000 examples)
MAX_STEPS = 100
LORA_RANK = 16
LORA_ALPHA = 16

# Medium datasets (1000-5000 examples)
MAX_STEPS = 500
LORA_RANK = 32
LORA_ALPHA = 32

# Large datasets (5000+ examples)
MAX_STEPS = 1000
LORA_RANK = 64
LORA_ALPHA = 64
```

### Key Hyperparameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `LORA_RANK` | 16 | Rank of adaptation matrices (higher = more capacity) |
| `LORA_ALPHA` | 16 | Scaling factor (usually equals rank) |
| `LEARNING_RATE` | 2e-4 | Training learning rate |
| `BATCH_SIZE` | 4 | Training batch size |
| `MAX_STEPS` | 100 | Total training steps |

---

## 7. Export & Deployment Steps

### Step 1: Export Training Data

```bash
# Via UI: Index page → LoRA Export → Run Export
# Or via API:
curl -X POST http://localhost:3010/training/export_snapshot
```

This runs:
1. `export_docs.py` - Extracts documents to JSONL
2. `normalize_dataset.py` - Converts to Alpaca format

### Step 2: Train on Google Colab

1. Open [Google Colab](https://colab.research.google.com)
2. Upload `notebooks/unsloth_finetune.py`
3. Upload your exported `training_data.jsonl`
4. Run all cells
5. Download the GGUF file and Modelfile

### Step 3: Import to Ollama

```bash
# Using the installer script
./scripts/custom_model.sh --path /path/to/model.gguf

# Or manually
cd /path/to/downloaded/files
ollama create ag-custom -f Modelfile
```

### Step 4: Enable Custom Model

```bash
export CUSTOM_MODEL_ENABLED=true
export CUSTOM_MODEL_NAME=ag-custom
export CUSTOM_MODEL_FALLBACK=true  # Fall back to default if unavailable
```

### Step 5: Verify Import

```bash
ollama list | grep ag-custom
```

---

## 8. Quality Validation

Before deploying to production, verify:

### Functional Tests

- [ ] Model responds coherently to domain questions
- [ ] Answers are grounded in retrieved context
- [ ] No hallucinations on known facts
- [ ] Handles edge cases gracefully

### Performance Tests

- [ ] Inference latency is acceptable (<5s for typical queries)
- [ ] Memory usage stays within limits
- [ ] No crashes under load

### Comparison Test

```bash
# Test custom model
ollama run ag-custom "What is the main purpose of this system?"

# Compare with base model
ollama run phi:latest "What is the main purpose of this system?"
```

The custom model should:
- Give more specific, domain-relevant answers
- Better utilize provided context
- Show fewer generic/hallucinated responses

---

## 9. Environment Variables Reference

| Variable | Default | Description |
|----------|---------|-------------|
| `TRAINING_DATA_ENABLED` | `false` | Enable training data collection |
| `TRAINING_DATA_PATH` | `~/.local/share/ag/training/raw_examples.jsonl` | Raw data storage |
| `TRAINING_MIN_QUALITY` | `3` | Minimum quality score to collect |
| `TRAINING_EXPORT_PATH` | `~/.local/share/ag/training/training_data.jsonl` | Export output |
| `LORA_EXPORT_ONLY` | (empty) | Comma-separated paths to limit export |
| `CUSTOM_MODEL_ENABLED` | `false` | Use custom model for inference |
| `CUSTOM_MODEL_NAME` | `ag-custom` | Model name in Ollama |
| `CUSTOM_MODEL_FALLBACK` | `true` | Fall back to default if unavailable |

---

## 10. Summary Checklist

Before starting LoRA training:

- [ ] **500+ quality training examples** collected
- [ ] **RAG context included** in training data
- [ ] **GPU access** available (Colab T4 is free)
- [ ] **Ollama installed** for deployment
- [ ] **Base model selected** that fits in available RAM

After training:

- [ ] **GGUF export** completed successfully
- [ ] **Model imported** to Ollama (`ollama list` shows it)
- [ ] **Validation passed** (comparison tests)
- [ ] **Environment configured** (`CUSTOM_MODEL_ENABLED=true`)

---

## Troubleshooting

### "Training data collection is disabled"

```bash
export TRAINING_DATA_ENABLED=true
# Then restart the backend
```

### "Not enough examples for training"

Keep collecting! Check progress:
```bash
curl http://localhost:3010/training/stats
```

### "Model import failed"

1. Check Ollama is running: `ollama list`
2. Verify GGUF file exists and is valid
3. Check Modelfile syntax

### "Custom model not found"

1. Verify model was imported: `ollama list | grep ag-custom`
2. Check model name matches `CUSTOM_MODEL_NAME`
3. If fallback is enabled, system will use default model

### "Out of memory during inference"

1. Use a smaller base model (Phi-3.5-mini recommended)
2. Use more aggressive quantization (Q4_0)
3. Reduce context window size

---

## References

- [Unsloth Documentation](https://unsloth.ai/docs)
- [Unsloth + Ollama Tutorial](https://unsloth.ai/docs/get-started/fine-tuning-llms-guide/tutorial-how-to-finetune-llama-3-and-use-in-ollama)
- [GGUF Quantization Guide](https://unsloth.ai/docs/basics/inference-and-deployment/saving-to-gguf)
- [Alpaca Dataset Format](https://github.com/tatsu-lab/stanford_alpaca)
- [PEFT Documentation](https://huggingface.co/docs/peft)
