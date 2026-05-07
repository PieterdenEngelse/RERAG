#!/bin/bash
set -euo pipefail

ROOT_DIR="/home/pde/ag"
COMPOSE_FILE="${ROOT_DIR}/docker-compose.yml"

cd "$ROOT_DIR"

if command -v docker >/dev/null 2>&1; then
  if [ -f "$COMPOSE_FILE" ]; then
    echo "[start-ag] Ensuring full observability stack is running..."
    docker compose -f "$COMPOSE_FILE" up -d redis tempo loki prometheus grafana otel-collector >/tmp/ag_full_stack.log 2>&1 || echo "[start-ag] Warning: failed to start full stack (see /tmp/ag_full_stack.log)"
  else
    echo "[start-ag] Warning: $COMPOSE_FILE not found; skipping stack startup"
  fi
else
  echo "[start-ag] Warning: docker not available; skipping stack startup"
fi

# Locate libtika_native.so (built by the extractous crate; build-hash is unstable).
# Pick the newest build dir so rebuilds don't break the link.
TIKA_LIBS_DIR=$(ls -td "$ROOT_DIR"/target/release/build/extractous-*/out/libs 2>/dev/null | head -n1 || true)
if [ -n "${TIKA_LIBS_DIR:-}" ] && [ -f "$TIKA_LIBS_DIR/libtika_native.so" ]; then
  export LD_LIBRARY_PATH="$TIKA_LIBS_DIR${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
else
  echo "[start-ag] Warning: libtika_native.so not found under target/release/build/extractous-*; release binary will fail to load."
fi

exec "$ROOT_DIR/target/release/ag"
