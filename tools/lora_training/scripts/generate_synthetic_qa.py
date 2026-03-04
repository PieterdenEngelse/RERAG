#!/usr/bin/env python3
"""
Generate synthetic Q&A training data from document chunks.

This script:
1. Reads exported document chunks (from export_docs.py)
2. Uses Ollama to generate realistic questions about each chunk
3. Retrieves context via the RAG API
4. Generates grounded answers using the retrieved context
5. Outputs training-ready JSONL

Usage:
    python generate_synthetic_qa.py [options]

Options:
    --input PATH          Input JSONL file (default: data/docs_snapshot.jsonl)
    --output PATH         Output JSONL file (default: data/synthetic_qa.jsonl)
    --questions-per-chunk N  Questions to generate per chunk (default: 3)
    --ollama-url URL      Ollama API URL (default: http://localhost:11434)
    --ollama-model MODEL  Model for generation (default: phi3.5:latest)
    --rag-url URL         RAG API URL (default: http://localhost:3010)
    --max-chunks N        Max chunks to process (default: all)
    --skip-existing       Skip if output file exists
    --verbose             Show progress details
"""

import argparse
import json
import pathlib
import sys
import time
from datetime import datetime
from typing import Generator, Optional

# Try to import requests, provide helpful error if missing
try:
    import requests
except ImportError:
    print("Error: 'requests' library not found.")
    print("Install it with: pip install requests")
    sys.exit(1)

ROOT = pathlib.Path(__file__).resolve().parents[3]
DEFAULT_INPUT = ROOT / "tools" / "lora_training" / "data" / "docs_snapshot.jsonl"
DEFAULT_OUTPUT = ROOT / "tools" / "lora_training" / "data" / "synthetic_qa.jsonl"

# Prompt templates
QUESTION_GENERATION_PROMPT = """You are helping create training data for a RAG system. Given the following document excerpt, generate {n} realistic questions that a user might ask about this content.

Requirements:
- Questions should be specific and answerable from the text
- Vary question types: how-to, what-is, why, troubleshooting, etc.
- Questions should sound natural, like a real user would ask
- Each question on its own line, numbered 1., 2., 3., etc.

Document excerpt:
---
{text}
---

Generate {n} questions:"""

ANSWER_GENERATION_PROMPT = """You are a helpful assistant for a RAG (Retrieval-Augmented Generation) system. Answer the user's question using ONLY the provided context. Be concise but complete.

If the context doesn't contain enough information to fully answer, say what you can based on the context and note what's missing.

Context:
---
{context}
---

Question: {question}

Answer:"""


def load_chunks(input_path: pathlib.Path) -> Generator[dict, None, None]:
    """Load document chunks from JSONL file."""
    if not input_path.exists():
        print(f"Error: Input file not found: {input_path}")
        print("Run export_docs.py first to create the document snapshot.")
        sys.exit(1)
    
    with input_path.open() as f:
        for line in f:
            if line.strip():
                yield json.loads(line)


def chunk_text(text: str, max_chars: int = 2000) -> list[str]:
    """Split text into smaller chunks for processing."""
    if len(text) <= max_chars:
        return [text]
    
    chunks = []
    paragraphs = text.split('\n\n')
    current_chunk = ""
    
    for para in paragraphs:
        if len(current_chunk) + len(para) + 2 <= max_chars:
            current_chunk += para + "\n\n"
        else:
            if current_chunk:
                chunks.append(current_chunk.strip())
            current_chunk = para + "\n\n"
    
    if current_chunk:
        chunks.append(current_chunk.strip())
    
    return chunks if chunks else [text[:max_chars]]


def call_ollama(
    prompt: str,
    ollama_url: str,
    model: str,
    timeout: int = 120
) -> Optional[str]:
    """Call Ollama API to generate text."""
    try:
        response = requests.post(
            f"{ollama_url}/api/generate",
            json={
                "model": model,
                "prompt": prompt,
                "stream": False,
                "options": {
                    "temperature": 0.7,
                    "num_predict": 500,
                }
            },
            timeout=timeout
        )
        response.raise_for_status()
        return response.json().get("response", "").strip()
    except requests.exceptions.RequestException as e:
        print(f"  Warning: Ollama request failed: {e}")
        return None


def search_rag(
    query: str,
    rag_url: str,
    timeout: int = 30
) -> Optional[str]:
    """Search RAG API and return context."""
    try:
        response = requests.get(
            f"{rag_url}/search",
            params={"q": query, "limit": 5},
            timeout=timeout
        )
        response.raise_for_status()
        results = response.json()
        
        # Extract text from search results
        contexts = []
        if isinstance(results, list):
            for r in results[:5]:
                if isinstance(r, dict):
                    text = r.get("text") or r.get("content") or r.get("chunk", "")
                    if text:
                        contexts.append(text)
                elif isinstance(r, str):
                    contexts.append(r)
        elif isinstance(results, dict):
            # Handle different response formats
            items = results.get("results") or results.get("chunks") or results.get("documents") or []
            for r in items[:5]:
                if isinstance(r, dict):
                    text = r.get("text") or r.get("content") or r.get("chunk", "")
                    if text:
                        contexts.append(text)
        
        return "\n\n---\n\n".join(contexts) if contexts else None
    except requests.exceptions.RequestException as e:
        print(f"  Warning: RAG search failed: {e}")
        return None


def generate_questions(
    text: str,
    n: int,
    ollama_url: str,
    model: str
) -> list[str]:
    """Generate questions about a text chunk."""
    prompt = QUESTION_GENERATION_PROMPT.format(text=text[:3000], n=n)
    response = call_ollama(prompt, ollama_url, model)
    
    if not response:
        return []
    
    # Parse numbered questions
    questions = []
    for line in response.split('\n'):
        line = line.strip()
        # Match patterns like "1.", "1)", "1:", or just numbered lines
        if line and (line[0].isdigit() or line.startswith('-') or line.startswith('•')):
            # Remove numbering/bullets
            for prefix in ['1.', '2.', '3.', '4.', '5.', '1)', '2)', '3)', '4)', '5)', '-', '•', '*']:
                if line.startswith(prefix):
                    line = line[len(prefix):].strip()
                    break
            if line and '?' in line:
                questions.append(line)
    
    return questions[:n]


def generate_answer(
    question: str,
    context: str,
    ollama_url: str,
    model: str
) -> Optional[str]:
    """Generate an answer using the provided context."""
    prompt = ANSWER_GENERATION_PROMPT.format(context=context[:4000], question=question)
    return call_ollama(prompt, ollama_url, model, timeout=180)


def main():
    parser = argparse.ArgumentParser(
        description="Generate synthetic Q&A training data from documents"
    )
    parser.add_argument(
        "--input", "-i",
        type=pathlib.Path,
        default=DEFAULT_INPUT,
        help=f"Input JSONL file (default: {DEFAULT_INPUT.relative_to(ROOT)})"
    )
    parser.add_argument(
        "--output", "-o",
        type=pathlib.Path,
        default=DEFAULT_OUTPUT,
        help=f"Output JSONL file (default: {DEFAULT_OUTPUT.relative_to(ROOT)})"
    )
    parser.add_argument(
        "--questions-per-chunk", "-n",
        type=int,
        default=3,
        help="Questions to generate per chunk (default: 3)"
    )
    parser.add_argument(
        "--ollama-url",
        default="http://localhost:11434",
        help="Ollama API URL (default: http://localhost:11434)"
    )
    parser.add_argument(
        "--ollama-model",
        default="phi3.5:latest",
        help="Model for generation (default: phi3.5:latest)"
    )
    parser.add_argument(
        "--rag-url",
        default="http://localhost:3010",
        help="RAG API URL (default: http://localhost:3010)"
    )
    parser.add_argument(
        "--max-chunks",
        type=int,
        default=None,
        help="Max chunks to process (default: all)"
    )
    parser.add_argument(
        "--skip-existing",
        action="store_true",
        help="Skip if output file exists"
    )
    parser.add_argument(
        "--verbose", "-v",
        action="store_true",
        help="Show progress details"
    )
    
    args = parser.parse_args()
    
    # Check if output exists
    if args.skip_existing and args.output.exists():
        print(f"Output file exists, skipping: {args.output}")
        return
    
    # Verify services are running
    print("Checking services...")
    try:
        requests.get(f"{args.ollama_url}/api/tags", timeout=5)
        print(f"  ✓ Ollama is running at {args.ollama_url}")
    except requests.exceptions.RequestException:
        print(f"  ✗ Ollama not reachable at {args.ollama_url}")
        print("    Start Ollama with: ollama serve")
        sys.exit(1)
    
    try:
        requests.get(f"{args.rag_url}/health", timeout=5)
        print(f"  ✓ RAG API is running at {args.rag_url}")
    except requests.exceptions.RequestException:
        print(f"  ✗ RAG API not reachable at {args.rag_url}")
        print("    Start the backend with: cd backend && cargo run")
        sys.exit(1)
    
    # Load documents
    print(f"\nLoading documents from {args.input}...")
    docs = list(load_chunks(args.input))
    print(f"  Found {len(docs)} documents")
    
    if args.max_chunks:
        docs = docs[:args.max_chunks]
        print(f"  Processing first {len(docs)} documents")
    
    # Process documents
    args.output.parent.mkdir(parents=True, exist_ok=True)
    
    total_examples = 0
    failed_questions = 0
    failed_answers = 0
    
    print(f"\nGenerating Q&A pairs...")
    print(f"  Model: {args.ollama_model}")
    print(f"  Questions per chunk: {args.questions_per_chunk}")
    print()
    
    with args.output.open("w", encoding="utf-8") as out_file:
        for doc_idx, doc in enumerate(docs):
            source = doc.get("source", "unknown")
            text = doc.get("text", "")
            
            if not text.strip():
                continue
            
            print(f"[{doc_idx + 1}/{len(docs)}] Processing: {source}")
            
            # Split into smaller chunks if needed
            text_chunks = chunk_text(text, max_chars=2000)
            
            for chunk_idx, chunk in enumerate(text_chunks):
                if args.verbose:
                    print(f"  Chunk {chunk_idx + 1}/{len(text_chunks)}")
                
                # Generate questions
                questions = generate_questions(
                    chunk,
                    args.questions_per_chunk,
                    args.ollama_url,
                    args.ollama_model
                )
                
                if not questions:
                    failed_questions += 1
                    if args.verbose:
                        print(f"    Warning: No questions generated")
                    continue
                
                if args.verbose:
                    print(f"    Generated {len(questions)} questions")
                
                # For each question, get RAG context and generate answer
                for q_idx, question in enumerate(questions):
                    # Search RAG for context
                    context = search_rag(question, args.rag_url)
                    
                    if not context:
                        # Fall back to using the original chunk as context
                        context = chunk
                    
                    # Generate answer
                    answer = generate_answer(
                        question,
                        context,
                        args.ollama_url,
                        args.ollama_model
                    )
                    
                    if not answer:
                        failed_answers += 1
                        if args.verbose:
                            print(f"    Warning: Failed to generate answer for Q{q_idx + 1}")
                        continue
                    
                    # Create training example
                    example = {
                        "instruction": question,
                        "context": context[:4000],  # Limit context size
                        "response": answer,
                        "source": source,
                        "timestamp": datetime.utcnow().isoformat(),
                        "tags": ["synthetic", "auto-generated"],
                        "metadata": {
                            "generator": "generate_synthetic_qa.py",
                            "model": args.ollama_model,
                            "chunk_index": chunk_idx,
                        }
                    }
                    
                    out_file.write(json.dumps(example, ensure_ascii=False) + "\n")
                    total_examples += 1
                    
                    if args.verbose:
                        print(f"    ✓ Q{q_idx + 1}: {question[:60]}...")
                
                # Small delay to avoid overwhelming the API
                time.sleep(0.5)
    
    # Summary
    print(f"\n{'='*50}")
    print(f"Generation complete!")
    print(f"  Total examples: {total_examples}")
    print(f"  Failed question generations: {failed_questions}")
    print(f"  Failed answer generations: {failed_answers}")
    print(f"  Output file: {args.output}")
    
    if total_examples >= 500:
        print(f"\n✓ You have enough examples for LoRA training!")
    else:
        needed = 500 - total_examples
        print(f"\n⚠ You need ~{needed} more examples to reach the 500 minimum.")
        print(f"  Try processing more documents or increasing --questions-per-chunk")


if __name__ == "__main__":
    main()
