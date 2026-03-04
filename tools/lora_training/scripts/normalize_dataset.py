#!/usr/bin/env python3
"""Normalize raw doc exports into instruction/context/response examples."""
import json
import pathlib
from pathlib import Path
from datetime import datetime
import os

ROOT = pathlib.Path(__file__).resolve().parents[3]
ONLY_FILES = {name.strip() for name in os.getenv('LORA_EXPORT_ONLY', '').split(',') if name.strip()}

RAW_PATH = ROOT / "tools" / "lora_training" / "data" / "docs_snapshot.jsonl"
DATASET_DIR = ROOT / "tools" / "lora_training" / "data" / f"dataset_{datetime.utcnow().date()}"


TEMPLATE = (
    "Summarize the key ideas from the following repository document. Focus on actionable guidance "
    "for engineers working on the retrieval stack."
)


def normalize_record(record: dict) -> dict:
    text = record["text"].strip()
    preview = text.splitlines()[:40]
    context = "\n".join(preview)
    response = text if len(text) < 4000 else text[:4000] + "\n..."

    return {
        "instruction": TEMPLATE,
        "context": context,
        "response": response,
        "source": record.get("source"),
        "timestamp": record.get("timestamp"),
        "tags": ["documentation"],
    }


def main():
    DATASET_DIR.mkdir(parents=True, exist_ok=True)
    records = []
    with RAW_PATH.open() as f:
        for line in f:
            record = json.loads(line)
            if ONLY_FILES and Path(record.get("source", "")).name not in ONLY_FILES:
                continue
            records.append(normalize_record(record))

    split_idx = max(1, int(len(records) * 0.8))
    splits = {
        "train.jsonl": records[:split_idx],
        "val.jsonl": records[split_idx:],
    }

    for name, items in splits.items():
        path = DATASET_DIR / name
        with path.open("w", encoding="utf-8") as f:
            for item in items:
                f.write(json.dumps(item, ensure_ascii=False) + "\n")

    print(f"Wrote dataset to {DATASET_DIR}")


if __name__ == "__main__":
    main()
