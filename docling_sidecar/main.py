"""
Docling sidecar — converts uploaded documents to DocIR JSON.

The Rust DoclingExtractor POSTs raw bytes to POST /convert as multipart.
This service calls the Docling library and returns JSON matching the DocIR
schema that Rust deserializes directly into crate::doc_ir::DocIR.

BlockType Serde format (must match Rust enum serialization exactly):
  unit variants:  "Text", "Formula", "Caption", "Footnote", "PageBreak"
  struct variants: {"Header": {"level": 1}}, {"Table": {"rows": 3, "cols": 4}},
                   {"Code": {"language": null}}, {"List": {"ordered": false}},
                   {"Image": {"alt": null}}
"""

from __future__ import annotations

import os
import tempfile
import uuid
from pathlib import Path
from typing import Any

from fastapi import FastAPI, File, HTTPException, UploadFile
from fastapi.responses import JSONResponse

app = FastAPI(title="ag-docling-sidecar", version="1.0.0")

# Lazy-import Docling so startup is fast; first /convert triggers the load.
_converter = None


def get_converter():
    global _converter
    if _converter is None:
        from docling.document_converter import DocumentConverter

        _converter = DocumentConverter()
    return _converter


# ── Helpers ──────────────────────────────────────────────────────────────────


def _bbox(item: Any) -> dict | None:
    """Extract first provenance bounding box as DocIR BoundingBox."""
    prov = getattr(item, "prov", None)
    if not prov:
        return None
    p = prov[0]
    bb = getattr(p, "bbox", None)
    if bb is None:
        return None
    return {
        "page": getattr(p, "page_no", 1),
        "x0": float(getattr(bb, "l", 0)),
        "y0": float(getattr(bb, "t", 0)),
        "x1": float(getattr(bb, "r", 0)),
        "y1": float(getattr(bb, "b", 0)),
    }


def _page(item: Any) -> int | None:
    prov = getattr(item, "prov", None)
    if not prov:
        return None
    return getattr(prov[0], "page_no", None)


def _label(item: Any) -> str:
    """Normalise Docling item label to a lowercase string."""
    label = getattr(item, "label", None)
    if label is None:
        return "paragraph"
    return str(label).lower().replace("doclabel.", "").replace("docitemlabel.", "")


def _block_type(label: str, item: Any) -> Any:
    """Map a Docling label to a DocIR BlockType value (Serde-compatible)."""
    if "section_header" in label or label in ("title", "section-header"):
        level = int(getattr(item, "level", 1) or 1)
        return {"Header": {"level": level}}
    if label in ("paragraph", "text", "body"):
        return "Text"
    if "table" in label:
        return {"Table": {"rows": 0, "cols": 0}}  # overridden below
    if "code" in label:
        return {"Code": {"language": None}}
    if "caption" in label:
        return "Caption"
    if "footnote" in label:
        return "Footnote"
    if "list" in label:
        ordered = "ordered" in label
        return {"List": {"ordered": ordered}}
    if "picture" in label or "figure" in label or "image" in label:
        return {"Image": {"alt": None}}
    if "formula" in label or "equation" in label:
        return "Formula"
    if "page_break" in label or "page-break" in label:
        return "PageBreak"
    return "Text"


def _make_block(block_type: Any, text: str, markdown: str | None, item: Any) -> dict:
    return {
        "id": str(uuid.uuid4()),
        "block_type": block_type,
        "text": text,
        "markdown": markdown,
        "bbox": _bbox(item),
        "page": _page(item),
        "metadata": {},
    }


# ── Routes ───────────────────────────────────────────────────────────────────


@app.get("/health")
async def health():
    return {"status": "ok"}


@app.post("/convert")
async def convert(file: UploadFile = File(...)):
    suffix = Path(file.filename or "document.pdf").suffix or ".pdf"
    data = await file.read()

    with tempfile.NamedTemporaryFile(suffix=suffix, delete=False) as tmp:
        tmp.write(data)
        tmp_path = tmp.name

    try:
        result = get_converter().convert(tmp_path)
        doc = result.document
        blocks: list[dict] = []

        # Walk text items in reading order
        for text_item in getattr(doc, "texts", []):
            label = _label(text_item)
            text = str(getattr(text_item, "text", "") or "").strip()
            if not text:
                continue

            bt = _block_type(label, text_item)
            md = None
            if isinstance(bt, dict) and "Header" in bt:
                level = bt["Header"]["level"]
                md = f"{'#' * level} {text}"

            blocks.append(_make_block(bt, text, md, text_item))

        # Walk tables
        for table_item in getattr(doc, "tables", []):
            grid = getattr(getattr(table_item, "data", None), "grid", None) or []
            rows = len(grid)
            cols = max((len(r) for r in grid), default=0)
            # Export to markdown if available
            md_text: str | None = None
            try:
                md_text = table_item.export_to_markdown()
            except Exception:
                pass
            flat_text = md_text or " | ".join(
                " | ".join(str(c) for c in row) for row in grid
            )
            bt = {"Table": {"rows": rows, "cols": cols}}
            blocks.append(_make_block(bt, flat_text, md_text, table_item))

        page_count: int | None = None
        try:
            page_count = doc.num_pages()
        except Exception:
            pass

        return JSONResponse(
            {
                "source": file.filename or "document",
                "content_type": suffix.lstrip("."),
                "blocks": blocks,
                "page_count": page_count,
                "metadata": {},
            }
        )

    except Exception as exc:
        raise HTTPException(status_code=500, detail=str(exc)) from exc
    finally:
        os.unlink(tmp_path)


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=int(os.getenv("PORT", "5001")))
