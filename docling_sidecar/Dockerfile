FROM python:3.11-slim

WORKDIR /app

# System libs Docling needs for PDF/image processing
RUN apt-get update && apt-get install -y --no-install-recommends \
    libgl1 \
    libglib2.0-0 \
    libgomp1 \
    poppler-utils \
    && rm -rf /var/lib/apt/lists/*

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Pre-download Docling models at build time so the first /convert is fast
RUN python -c "from docling.document_converter import DocumentConverter; DocumentConverter()"

COPY main.py .

ENV PORT=5001
EXPOSE 5001

CMD ["uvicorn", "main:app", "--host", "0.0.0.0", "--port", "5001", "--workers", "2"]
