#!/bin/bash
# Kill any stale ag processes before startup.
#
# fuser 3010/tcp silently returns empty inside the systemd ExecStartPre sandbox
# (PrivateTmp=true + ProtectSystem=full restrict /proc/*/fd visibility for other
# processes). pkill works because it signals via kill(2), which is unaffected by
# those restrictions.
#
# Three-pass approach:
#   1. pkill -x ag — kills all user-owned ag processes by name (catches in-cgroup)
#   2. ss + kill — kills by port owner PID, catches PPID=1 orphans pkill misses
#   3. Poll ss until port 3010 is free — handles TIME_WAIT and slow OS cleanup

echo "[ag.service pre-start] killing stale ag processes..."
pkill -9 -x ag 2>/dev/null || true

# Kill any process holding port 3010 or 3011, regardless of cgroup or name.
# ss -tlnp emits "users:(("ag",pid=NNN,fd=M))" when the listener is visible.
for port in 3010 3011; do
    pid=$(ss -tlnpH "sport = :${port}" 2>/dev/null \
          | grep -oP 'pid=\K[0-9]+' | head -1)
    if [ -n "$pid" ]; then
        echo "[ag.service pre-start] killing PID $pid holding port $port..."
        kill -9 "$pid" 2>/dev/null || true
    fi
done

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
