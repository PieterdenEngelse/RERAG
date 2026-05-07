#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")/backend"

echo "[deploy] building release binary..."
cargo build --release 2>&1 | grep -E "^error|Finished|Compiling ag "

echo "[deploy] restarting ag.service..."
sudo systemctl restart ag.service

echo "[deploy] waiting for service..."
for i in $(seq 1 15); do
    state=$(sudo systemctl is-active ag.service 2>/dev/null)
    if [ "$state" = "active" ]; then
        echo "[deploy] ag.service is running"
        exit 0
    elif [ "$state" = "failed" ]; then
        echo "[deploy] ag.service failed to start"
        sudo journalctl -u ag.service -n 10 --no-pager
        exit 1
    fi
    sleep 1
done

echo "[deploy] timed out waiting for ag.service"
sudo systemctl status ag.service --no-pager | head -6
exit 1
