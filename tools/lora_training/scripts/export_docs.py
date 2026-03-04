#!/usr/bin/env python3
"""Placeholder exporter for documentation -> JSONL dataset."""
import json
import pathlib
from datetime import datetime
import os

ROOT = pathlib.Path(__file__).resolve().parents[3]
ONLY_FILES = {name.strip() for name in os.getenv('LORA_EXPORT_ONLY', '').split(',') if name.strip()}
DOCS_DIR = ROOT / "documents"
OUTPUT = ROOT / "tools" / "lora_training" / "data" / "docs_snapshot.jsonl"


def collect_docs():
    for path in DOCS_DIR.rglob("*.md"):
        if ONLY_FILES and path.name not in ONLY_FILES:
            continue
        text = path.read_text(encoding="utf-8")
        yield {
            "source": str(path.relative_to(ROOT)),
            "timestamp": datetime.utcnow().isoformat(),
            "text": text,
        }


def main():
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    with OUTPUT.open("w", encoding="utf-8") as f:
        for record in collect_docs():
            f.write(json.dumps(record, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
