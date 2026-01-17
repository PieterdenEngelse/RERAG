#!/usr/bin/env python3
"""
Convert HuggingFace embedding model to ONNX format.

Usage:
    pip install transformers torch onnx
    python scripts/convert_to_onnx.py

Output:
    models/embedding_model.onnx
"""

import os
import torch
from transformers import AutoModel, AutoTokenizer

def convert_to_onnx(
    model_name: str = "sentence-transformers/all-MiniLM-L6-v2",
    output_path: str = "models/embedding_model.onnx",
    max_length: int = 512
):
    print(f"Loading model: {model_name}")
    model = AutoModel.from_pretrained(model_name)
    tokenizer = AutoTokenizer.from_pretrained(model_name)
    model.eval()

    # Create output directory
    os.makedirs(os.path.dirname(output_path), exist_ok=True)

    # Create dummy input
    dummy_text = "This is a sample sentence for ONNX export."
    inputs = tokenizer(
        dummy_text,
        return_tensors="pt",
        padding="max_length",
        truncation=True,
        max_length=max_length
    )

    print(f"Exporting to ONNX: {output_path}")
    
    # Export to ONNX
    torch.onnx.export(
        model,
        (inputs["input_ids"], inputs["attention_mask"]),
        output_path,
        input_names=["input_ids", "attention_mask"],
        output_names=["last_hidden_state"],
        dynamic_axes={
            "input_ids": {0: "batch_size", 1: "sequence"},
            "attention_mask": {0: "batch_size", 1: "sequence"},
            "last_hidden_state": {0: "batch_size", 1: "sequence"}
        },
        opset_version=14,
        do_constant_folding=True
    )

    # Verify the model
    import onnx
    onnx_model = onnx.load(output_path)
    onnx.checker.check_model(onnx_model)
    
    file_size = os.path.getsize(output_path) / (1024 * 1024)
    print(f"✅ Model exported successfully!")
    print(f"   Path: {output_path}")
    print(f"   Size: {file_size:.1f} MB")
    print(f"\nTo use in ag:")
    print(f"   export EMBEDDING_PROVIDER=onnx")
    print(f"   export ONNX_MODEL_PATH={output_path}")

if __name__ == "__main__":
    convert_to_onnx()
