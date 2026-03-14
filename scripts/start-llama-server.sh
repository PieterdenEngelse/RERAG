#!/usr/bin/env bash
DB="$HOME/.local/share/ag/db/documents.db"
BINARY="$HOME/llama.cpp/build/bin/llama-server"
MODELS_DIR="$HOME/llama.cpp/models"

if [ ! -f "$DB" ]; then echo "ERROR: ag database not found at $DB"; exit 1; fi
if [ ! -f "$BINARY" ]; then echo "ERROR: llama-server binary not found at $BINARY"; exit 1; fi

CONFIG_JSON=$(sqlite3 "$DB" "SELECT config_json FROM app_config WHERE config_type='hardware';" 2>/dev/null)
if [ -z "$CONFIG_JSON" ]; then echo "ERROR: No hardware config found in DB"; exit 1; fi

MODEL=$(echo "$CONFIG_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('model',''))")
NUM_CTX=$(echo "$CONFIG_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('num_ctx', 2048))")
NUM_THREAD=$(echo "$CONFIG_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('num_thread', 4))")
LLAMA_URL=$(echo "$CONFIG_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('llama_server_url','http://127.0.0.1:11435'))")
PORT=$(echo "$LLAMA_URL" | python3 -c "import sys; url=sys.stdin.read().strip(); print(url.split(':')[-1])")

if [ -f "$MODEL" ]; then MODEL_PATH="$MODEL"
elif [ -f "$MODELS_DIR/$MODEL" ]; then MODEL_PATH="$MODELS_DIR/$MODEL"
elif [ -f "$MODELS_DIR/${MODEL}.gguf" ]; then MODEL_PATH="$MODELS_DIR/${MODEL}.gguf"
else echo "ERROR: Model '$MODEL' not found in $MODELS_DIR"; exit 1; fi

echo "Starting llama-server: model=$MODEL_PATH port=$PORT threads=$NUM_THREAD ctx=$NUM_CTX"
exec "$BINARY" -m "$MODEL_PATH" --host 127.0.0.1 --port "$PORT" --threads "$NUM_THREAD" --ctx-size "$NUM_CTX"
