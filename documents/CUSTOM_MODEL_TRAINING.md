# Custom Model Training Guide

## Overview

Phase 20 introduces the ability to fine-tune custom LLM models for your specific RAG use case using Unsloth, then deploy them through Ollama.

**Key Concept: Train Elsewhere, Deploy Locally**
- Training requires GPU (use free Google Colab T4)
- Deployment uses your existing Ollama setup (8GB RAM compatible)

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    TRAINING PIPELINE                         │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  1. COLLECT DATA          2. TRAIN (CLOUD)                  │
│  ┌─────────────┐          ┌─────────────┐                   │
│  │ RAG Usage   │ ──────▶  │ Unsloth     │                   │
│  │ + Feedback  │          │ Fine-tune   │                   │
│  └─────────────┘          └─────────────┘                   │
│        │                        │                            │
│        ▼                        ▼                            │
│  3. EXPORT                 4. DEPLOY (LOCAL)                │
│  ┌─────────────┐          ┌─────────────┐                   │
│  │ JSONL File  │ ──────▶  │ Ollama      │                   │
│  │ (Alpaca)    │          │ Import      │                   │
│  └─────────────┘          └─────────────┘                   │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Quick Start

### Step 1: Enable Training Data Collection

```bash
# Add to your environment
export TRAINING_DATA_ENABLED=true
export TRAINING_MIN_QUALITY=3  # Only collect score >= 3
```

### Step 2: Collect Training Data

As you use the RAG system, provide feedback on responses:

```bash
# Via API
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

Or use the thumbs up/down buttons in the UI (when implemented).

### Step 3: Check Collection Progress

```bash
curl http://localhost:3010/training/stats
```

Response:
```json
{
  "status": "ok",
  "stats": {
    "total_examples": 150,
    "high_quality_count": 120,
    "usable_count": 145,
    "average_quality": 4.2,
    "ready_for_export": false
  },
  "collection_enabled": true
}
```

**Goal:** Collect 500+ usable examples for meaningful fine-tuning.

### Step 4: Export Training Data

```bash
curl -X POST http://localhost:3010/training/export
```

This creates a JSONL file in Alpaca format:
```json
{"instruction": "What is Rust?", "input": "Context...", "output": "Response..."}
```

### Step 5: Fine-tune with Unsloth

1. Open Google Colab (free T4 GPU)
2. Upload `notebooks/unsloth_finetune.py`
3. Upload your exported `training_data.jsonl`
4. Run the notebook
5. Download the GGUF file and Modelfile

### Step 6: Deploy to Ollama

```bash
# Using the installer script
./scripts/custom_model.sh --path /path/to/model.gguf

# Or manually
cd /path/to/downloaded/files
ollama create ag-custom -f Modelfile
```

### Step 7: Enable Custom Model

```bash
export CUSTOM_MODEL_ENABLED=true
export CUSTOM_MODEL_NAME=ag-custom
```

## API Reference

### POST /training/feedback

Submit user feedback for training data collection.

**Request:**
```json
{
  "query": "string",           // User's question
  "response": "string",        // Model's response
  "context": "string|null",    // RAG context (optional)
  "quality_score": 1-5,        // User rating
  "conversation_id": "string", // Optional grouping
  "mode": "rag|llm|hybrid",    // Chat mode used
  "model": "string"            // Model that generated response
}
```

**Response:**
```json
{
  "status": "collected|skipped|error",
  "example_id": "uuid",
  "message": "string",
  "request_id": "string"
}
```

### GET /training/stats

Get training data collection statistics.

**Response:**
```json
{
  "status": "ok",
  "stats": {
    "total_examples": 150,
    "high_quality_count": 120,
    "usable_count": 145,
    "average_quality": 4.2,
    "ready_for_export": false,
    "by_mode": {"hybrid": 100, "rag": 30, "llm": 20},
    "last_collected": "2025-01-11T10:30:00Z"
  },
  "collection_enabled": true
}
```

### POST /training/export

Export collected data for Unsloth training.

**Response:**
```json
{
  "status": "ok",
  "exported_count": 145,
  "output_path": "/home/user/.local/share/ag/training/training_data.jsonl",
  "message": "Exported 145 examples for Unsloth training"
}
```

### POST /training/clear

Clear all collected training data.

**Response:**
```json
{
  "status": "ok",
  "message": "Training data cleared"
}
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `TRAINING_DATA_ENABLED` | `false` | Enable training data collection |
| `TRAINING_DATA_PATH` | `~/.local/share/ag/training/raw_examples.jsonl` | Where to store raw data |
| `TRAINING_MIN_QUALITY` | `3` | Minimum quality score to collect |
| `TRAINING_EXPORT_PATH` | `~/.local/share/ag/training/training_data.jsonl` | Export output path |
| `CUSTOM_MODEL_ENABLED` | `false` | Use custom model instead of default |
| `CUSTOM_MODEL_NAME` | `ag-custom` | Model name in Ollama |
| `CUSTOM_MODEL_FALLBACK` | `true` | Fall back to default if custom unavailable |

## Model Recommendations

For 8GB RAM deployment:

| Model | Parameters | GGUF Size | RAM Usage | Notes |
|-------|------------|-----------|-----------|-------|
| **Phi-3.5-mini** | 3.8B | ~2.2GB | ~4GB | ✅ Recommended |
| Llama-3.2-1B | 1B | ~0.6GB | ~2GB | Smallest, fastest |
| Llama-3.2-3B | 3B | ~1.8GB | ~3.5GB | Good balance |
| Gemma-2-2B | 2B | ~1.2GB | ~3GB | 8K context only |

## Training Tips

### Data Quality

- **Aim for 500+ examples** minimum
- **Quality over quantity** - filter low scores
- **Diverse examples** - cover different query types
- **Include context** - RAG context helps the model learn retrieval patterns

### Training Configuration

```python
# In unsloth_finetune.py

# For small datasets (500-1000 examples)
MAX_STEPS = 100
LORA_RANK = 16

# For larger datasets (1000+ examples)
MAX_STEPS = 500
LORA_RANK = 32

# For very large datasets (5000+ examples)
MAX_STEPS = 1000
LORA_RANK = 64
```

### Evaluation

Before deploying, test your model:

```bash
# Quick test
ollama run ag-custom "What is the main purpose of this system?"

# Compare with base model
ollama run phi:latest "What is the main purpose of this system?"
```

## Troubleshooting

### "Training data collection is disabled"

Enable it:
```bash
export TRAINING_DATA_ENABLED=true
```

### "Not enough examples for training"

Keep collecting! You need 500+ usable examples. Check progress:
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

## Files

| File | Purpose |
|------|---------|
| `backend/src/training/mod.rs` | Training module |
| `backend/src/training/data_collector.rs` | Data collection logic |
| `backend/src/training/feedback.rs` | Quality scoring |
| `notebooks/unsloth_finetune.py` | Colab training script |
| `scripts/custom_model.sh` | Model import script |

## References

- [Unsloth Documentation](https://unsloth.ai/docs)
- [Unsloth + Ollama Tutorial](https://unsloth.ai/docs/get-started/fine-tuning-llms-guide/tutorial-how-to-finetune-llama-3-and-use-in-ollama)
- [GGUF Quantization Guide](https://unsloth.ai/docs/basics/inference-and-deployment/saving-to-gguf)
- [Alpaca Dataset Format](https://github.com/tatsu-lab/stanford_alpaca)
