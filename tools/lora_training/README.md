# LoRA Training & Adapter Export

This folder hosts scripts and notes for training LoRA adapters with PyTorch/PEFT and exporting them for Candle-based inference.

## Step 1 – Decide What to Fine-Tune On
- **Primary sources:** Tantivy/LanceDB chunks (production retrieval data), markdown/docs under `documents/`, and notable support conversations.
- **Task framing:** Instruction→response pairs where the instruction is the user query and the response is the ideal grounded answer using retrieved context.
- **Metadata to keep:** source path, chunk_id, timestamp, tags (e.g., `monitoring`, `neo4j`, `observability`).

## Step 2 – Export Source Data
- Use scripts under `scripts/` (e.g., `export_docs.py`) to dump text corpora to JSONL. Set `LORA_EXPORT_ONLY="file1.md,file2.md"` to limit exports to specific docs.
- Each exporter should normalize paths relative to repo root and stamp an ISO timestamp.
- Store snapshots in `data/<dataset_name>.jsonl` and version via filename (e.g., `docs_snapshot_2025-02-11.jsonl`).

## Workflow Overview

1. **Prepare data**
   - Export domain documents or conversation transcripts from Tantivy/LanceDB.
   - Normalize into JSON/CSV under `data/` (create subfolder here). Include metadata about the export date and filters.

2. **Configure training**
   - Place training configs in `configs/` (YAML or JSON).
   - Key fields: base model path, learning rate, LoRA rank/alpha, training batch size, max steps/epochs.

3. **Train adapter**
   - Use notebooks or scripts in `scripts/` (see `train_lora.py` placeholder) to fine-tune adapters.
   - Activate the repo `.venv` before running:
     ```bash
     cd /home/pde/ag
     source .venv/bin/activate
     python tools/lora_training/scripts/train_lora.py --config configs/support.json
     ```

4. **Export for Candle**
   - Save adapters via PEFT in safetensors format:
     ```python
     peft_model.save_pretrained("artifacts/support_adapter", safe_serialization=True)
     ```
   - Keep `adapter.safetensors`, `adapter_config.json`, and a short `README.md` per adapter describing training data + metrics.

5. **Organize artifacts**
   - Store outputs under `artifacts/<base_model>/<adapter_name>/`.
   - Example:
     ```
     tools/lora_training/artifacts/
       phi-2/
         support/
           adapter.safetensors
           adapter_config.json
           README.md
     ```

6. **Wire into backend**
   - Update backend env/configs to point to the adapter directories.
   - Ensure Candle loader knows how to read safetensors and merge LoRA weights at runtime.

## Step 3 – Normalize Dataset Format
- After exporting raw text, convert to instruction/response JSONL via `normalize_dataset.py` (to be added).
- Recommended schema:
  ```json
  {
    "instruction": "Question",
    "context": "Retrieved chunk(s)",
    "response": "Answer grounded in docs",
    "source": "documents/guide.md",
    "tags": ["monitoring", "neo4j"]
  }
  ```

## Step 4 – Splits & Documentation
- Store datasets under `data/<name>/` with `train.jsonl`, `val.jsonl`.
- Add a README describing export date, filters, preprocessing.

## Step 5 – Tokenization Checks
- Use `tokenizer_check.py` to ensure sequences fit context window.

## Step 6 – Version Control Strategy
- Large datasets stay out of git; add `.gitignore` entries (see `data/.gitignore`).
- Track metadata/configs in repo; store raw data on disk or object storage.
