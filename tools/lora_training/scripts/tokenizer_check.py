#!/usr/bin/env python3
"""Report token lengths for normalized datasets."""
import json
import pathlib
from statistics import mean

from transformers import AutoTokenizer

ROOT = pathlib.Path(__file__).resolve().parents[3]
DATASET_DIR = ROOT / "tools" / "lora_training" / "data" / "dataset_2026-02-16"
MODEL_NAME = "meta-llama/Llama-2-7b-chat-hf"
CONTEXT_LIMIT = 4096

def load_records(path):
    with path.open() as f:
        for line in f:
            yield json.loads(line)


def main():
    tokenizer = AutoTokenizer.from_pretrained(MODEL_NAME)
    report = {}
    for split in ["train.jsonl", "val.jsonl"]:
        path = DATASET_DIR / split
        lengths = []
        overflow = 0
        for record in load_records(path):
            text = "\n\n".join([record["instruction"], record["context"], record["response"]])
            length = len(tokenizer(text).input_ids)
            lengths.append(length)
            if length > CONTEXT_LIMIT:
                overflow += 1
        report[split] = {
            "count": len(lengths),
            "max_tokens": max(lengths) if lengths else 0,
            "mean_tokens": int(mean(lengths)) if lengths else 0,
            ">4096": overflow,
        }

    for split, stats in report.items():
        print(split, stats)


if __name__ == "__main__":
    main()
