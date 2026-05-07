#!/bin/bash
# Kill any stale ag processes before startup.
#
# fuser 3010/tcp silently returns empty inside the systemd ExecStartPre sandbox
# (PrivateTmp=true + ProtectSystem=full restrict /proc/*/fd visibility for other
# processes). pkill works because it signals via kill(2), which is unaffected by
# those restrictions.
#
# Two-pass approach:
#   1. pkill -x ag — kills all user-owned ag processes by name (fast, sandbox-safe)
#   2. Poll ss until port 3010 is free — handles TIME_WAIT and slow OS cleanup

echo "[ag.service pre-start] killing stale ag processes..."
pkill -9 -x ag 2>/dev/null || true

for i in $(seq 1 15); do
    if ! ss -tlnH sport = :3010 | grep -q .; then
        echo "[ag.service pre-start] port 3010 is free (attempt $i)"
        exit 0
    fi
    echo "[ag.service pre-start] waiting for port 3010 to clear (attempt $i)..."
    sleep 1
done

echo "[ag.service pre-start] timeout: port 3010 still occupied after 15s"
exit 1
