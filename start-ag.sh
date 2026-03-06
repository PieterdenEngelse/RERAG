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

exec "$ROOT_DIR/target/release/ag"
