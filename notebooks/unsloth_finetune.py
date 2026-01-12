# notebooks/unsloth_finetune.py
# Version: 1.0.0
# 
# Agentic RAG Custom Model Fine-tuning with Unsloth
# ================================================
# 
# This script fine-tunes a small LLM for your RAG system using Unsloth.
# Run in Google Colab with T4 GPU (free tier) or locally with CUDA.
#
# Prerequisites:
# - Upload your training_data.jsonl to Colab (exported from /training/export)
# - Select Runtime > Change runtime type > T4 GPU
#
# Usage:
# 1. Export training data: curl -X POST http://localhost:3010/training/export
# 2. Upload the exported JSONL file to Colab
# 3. Run this notebook
# 4. Download the GGUF file and Modelfile
# 5. Import to Ollama: ollama create ag-custom -f Modelfile

# ============================================================================
# Cell 1: Install Unsloth (run this first, then restart runtime if needed)
# ============================================================================

# Uncomment these lines when running in Colab:
# !pip install "unsloth[colab-new] @ git+https://github.com/unslothai/unsloth.git"
# !pip install --no-deps trl peft accelerate bitsandbytes

# ============================================================================
# Cell 2: Configuration
# ============================================================================

# Model selection - choose based on your deployment RAM:
# - unsloth/Phi-3.5-mini-instruct-bnb-4bit (3.8B params, ~2GB GGUF) - RECOMMENDED
# - unsloth/Llama-3.2-1B-Instruct-bnb-4bit (1B params, ~0.6GB GGUF) - Smallest
# - unsloth/Llama-3.2-3B-Instruct-bnb-4bit (3B params, ~1.8GB GGUF) - Good balance
# - unsloth/gemma-2-2b-it-bnb-4bit (2B params, ~1.2GB GGUF) - Alternative

MODEL_NAME = "unsloth/Phi-3.5-mini-instruct-bnb-4bit"
MAX_SEQ_LENGTH = 2048
LOAD_IN_4BIT = True

# Training configuration
TRAINING_FILE = "training_data.jsonl"  # Your exported training data
OUTPUT_DIR = "ag-custom-model"
MAX_STEPS = 100  # Increase to 500-1000 for better results
LEARNING_RATE = 2e-4
BATCH_SIZE = 2
GRADIENT_ACCUMULATION = 4

# LoRA configuration
LORA_RANK = 16  # Increase to 32 or 64 for more expressiveness
LORA_ALPHA = 16
LORA_DROPOUT = 0

# Quantization for export (for 8GB RAM deployment)
# Options: "q4_k_m" (recommended), "q8_0" (higher quality), "q2_k" (smallest)
QUANTIZATION = "q4_k_m"

# ============================================================================
# Cell 3: Import and Load Model
# ============================================================================

from unsloth import FastLanguageModel
import torch

print(f"Loading model: {MODEL_NAME}")
print(f"Max sequence length: {MAX_SEQ_LENGTH}")
print(f"4-bit quantization: {LOAD_IN_4BIT}")

model, tokenizer = FastLanguageModel.from_pretrained(
    model_name=MODEL_NAME,
    max_seq_length=MAX_SEQ_LENGTH,
    load_in_4bit=LOAD_IN_4BIT,
    dtype=None,  # Auto-detect (float16 for T4, bfloat16 for A100)
)

print("✅ Model loaded successfully!")

# ============================================================================
# Cell 4: Add LoRA Adapters
# ============================================================================

print(f"Adding LoRA adapters (rank={LORA_RANK}, alpha={LORA_ALPHA})")

model = FastLanguageModel.get_peft_model(
    model,
    r=LORA_RANK,
    target_modules=[
        "q_proj", "k_proj", "v_proj", "o_proj",
        "gate_proj", "up_proj", "down_proj"
    ],
    lora_alpha=LORA_ALPHA,
    lora_dropout=LORA_DROPOUT,
    bias="none",
    use_gradient_checkpointing="unsloth",  # Reduces memory usage
    random_state=3407,
)

print("✅ LoRA adapters added!")

# ============================================================================
# Cell 5: Prepare Dataset
# ============================================================================

from datasets import load_dataset

print(f"Loading training data from: {TRAINING_FILE}")

# Load the exported training data
dataset = load_dataset("json", data_files=TRAINING_FILE, split="train")
print(f"Loaded {len(dataset)} training examples")

# Alpaca prompt template - matches the RAG system's output format
# This template is used during training and must match the Modelfile template
alpaca_prompt = """Below is an instruction that describes a task, paired with an input that provides further context. Write a response that appropriately completes the request.

### Instruction:
{}

### Input:
{}

### Response:
{}"""

def formatting_prompts_func(examples):
    """Format examples into the Alpaca template for training."""
    instructions = examples["instruction"]
    inputs = examples["input"]
    outputs = examples["output"]
    texts = []
    
    for instruction, input_text, output in zip(instructions, inputs, outputs):
        # Format with the Alpaca template
        text = alpaca_prompt.format(instruction, input_text, output)
        # Add EOS token to mark end of response
        text = text + tokenizer.eos_token
        texts.append(text)
    
    return {"text": texts}

# Apply formatting
dataset = dataset.map(formatting_prompts_func, batched=True)
print("✅ Dataset formatted for training!")

# Show a sample
print("\n--- Sample Training Example ---")
print(dataset[0]["text"][:500] + "...")

# ============================================================================
# Cell 6: Training Configuration
# ============================================================================

from trl import SFTTrainer
from transformers import TrainingArguments

print(f"\nTraining configuration:")
print(f"  - Max steps: {MAX_STEPS}")
print(f"  - Batch size: {BATCH_SIZE}")
print(f"  - Gradient accumulation: {GRADIENT_ACCUMULATION}")
print(f"  - Effective batch size: {BATCH_SIZE * GRADIENT_ACCUMULATION}")
print(f"  - Learning rate: {LEARNING_RATE}")

trainer = SFTTrainer(
    model=model,
    tokenizer=tokenizer,
    train_dataset=dataset,
    dataset_text_field="text",
    max_seq_length=MAX_SEQ_LENGTH,
    dataset_num_proc=2,
    packing=False,  # Can enable for shorter sequences
    args=TrainingArguments(
        per_device_train_batch_size=BATCH_SIZE,
        gradient_accumulation_steps=GRADIENT_ACCUMULATION,
        warmup_steps=5,
        max_steps=MAX_STEPS,
        learning_rate=LEARNING_RATE,
        fp16=not torch.cuda.is_bf16_supported(),
        bf16=torch.cuda.is_bf16_supported(),
        logging_steps=10,
        optim="adamw_8bit",
        weight_decay=0.01,
        lr_scheduler_type="linear",
        seed=3407,
        output_dir=OUTPUT_DIR,
        report_to="none",  # Disable wandb/tensorboard
    ),
)

print("✅ Trainer configured!")

# ============================================================================
# Cell 7: Train!
# ============================================================================

print("\n🚀 Starting training...")
print("This may take 10-30 minutes depending on dataset size and max_steps.\n")

trainer_stats = trainer.train()

print(f"\n✅ Training completed!")
print(f"   - Runtime: {trainer_stats.metrics['train_runtime']:.2f} seconds")
print(f"   - Samples/second: {trainer_stats.metrics['train_samples_per_second']:.2f}")
print(f"   - Final loss: {trainer_stats.metrics.get('train_loss', 'N/A')}")

# ============================================================================
# Cell 8: Export to GGUF for Ollama
# ============================================================================

print(f"\n📦 Exporting model to GGUF format ({QUANTIZATION})...")
print("This may take a few minutes.\n")

model.save_pretrained_gguf(
    OUTPUT_DIR,
    tokenizer,
    quantization_method=QUANTIZATION
)

print(f"✅ Model exported to: {OUTPUT_DIR}/")

# List exported files
import os
for f in os.listdir(OUTPUT_DIR):
    size = os.path.getsize(os.path.join(OUTPUT_DIR, f)) / (1024 * 1024)
    print(f"   - {f} ({size:.1f} MB)")

# ============================================================================
# Cell 9: Create Ollama Modelfile
# ============================================================================

# Find the GGUF file
gguf_files = [f for f in os.listdir(OUTPUT_DIR) if f.endswith('.gguf')]
if gguf_files:
    gguf_filename = gguf_files[0]
else:
    gguf_filename = f"ag-custom-model-{QUANTIZATION}.gguf"

modelfile_content = f'''FROM ./{gguf_filename}

TEMPLATE """Below is an instruction that describes a task, paired with an input that provides further context. Write a response that appropriately completes the request.

### Instruction:
{{{{ .Prompt }}}}

### Input:
{{{{ .Context }}}}

### Response:
"""

PARAMETER temperature 0.7
PARAMETER top_p 0.9
PARAMETER stop "### Instruction:"
PARAMETER stop "### Input:"
PARAMETER stop "### Response:"
'''

modelfile_path = os.path.join(OUTPUT_DIR, "Modelfile")
with open(modelfile_path, "w") as f:
    f.write(modelfile_content)

print(f"✅ Modelfile created: {modelfile_path}")

# ============================================================================
# Cell 10: Deployment Instructions
# ============================================================================

print("\n" + "="*60)
print("🎉 TRAINING COMPLETE!")
print("="*60)
print(f"""
To deploy your custom model:

1. Download these files from {OUTPUT_DIR}/:
   - {gguf_filename}
   - Modelfile

2. Place them in the same directory on your local machine

3. Import to Ollama:
   cd /path/to/downloaded/files
   ollama create ag-custom -f Modelfile

4. Test the model:
   ollama run ag-custom "What is Rust?"

5. Enable in your RAG system:
   Set CUSTOM_MODEL_ENABLED=true
   Set CUSTOM_MODEL_NAME=ag-custom

The model will automatically be used for all RAG queries!
""")

# ============================================================================
# Optional: Test the model before export
# ============================================================================

def test_model(prompt, context=""):
    """Test the fine-tuned model with a sample prompt."""
    FastLanguageModel.for_inference(model)
    
    formatted = alpaca_prompt.format(prompt, context, "")
    inputs = tokenizer([formatted], return_tensors="pt").to("cuda")
    
    outputs = model.generate(
        **inputs,
        max_new_tokens=256,
        temperature=0.7,
        top_p=0.9,
    )
    
    response = tokenizer.decode(outputs[0], skip_special_tokens=True)
    # Extract just the response part
    if "### Response:" in response:
        response = response.split("### Response:")[-1].strip()
    
    return response

# Uncomment to test:
# print("\n--- Model Test ---")
# result = test_model("What is the main purpose of this RAG system?")
# print(result)
