#!/bin/bash
# ~/ag/tmux_startup.sh
# Version: 2.1
# Initialize tmux session "main" with standard windows
# Handles all tmux session/window management and auto-attach
# Fixed: q login/gui handling with fallback support
set -euo pipefail

LOG_FILE="$HOME/.tmux_startup.log"

{
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] Starting tmux session setup v2.1"
    
    SESSION="main"
    AG_HOME="$HOME/ag"
    SESSION_CREATED=0

    # Helper function to create a window only if it doesn't exist
    create_window_if_missing() {
        local window_name="$1"
        local window_path="$2"
        if tmux list-windows -t "$SESSION" -F "#{window_name}" 2>/dev/null | grep -q "^${window_name}$"; then
            echo "[$(date '+%Y-%m-%d %H:%M:%S')] Window $window_name already exists"
            return 0
        else
            echo "[$(date '+%Y-%m-%d %H:%M:%S')] Creating window $window_name"
            tmux new-window -t "$SESSION:" -n "$window_name" -c "$window_path" -d
        fi
    }

    # Create session if it doesn't exist
    if ! tmux has-session -t "$SESSION" 2>/dev/null; then
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] Creating new tmux session $SESSION"
        tmux new-session -s "$SESSION" -d -c "$AG_HOME" -n "Q"
        SESSION_CREATED=1
    fi

    # Create named windows
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] Creating windows..."
    create_window_if_missing "pro" "$AG_HOME"
    create_window_if_missing "gra" "$AG_HOME"
    create_window_if_missing "otl" "$AG_HOME"
    create_window_if_missing "tmu" "$HOME"
    create_window_if_missing "ag" "$HOME"
    create_window_if_missing "tem" "$HOME"
    create_window_if_missing "q2" "$HOME"

    # Send commands to specific windows
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] Setting up window commands"

    # Q window: auto-run q login/gui only once per new tmux session
    if tmux list-windows -t "$SESSION" | grep -q "Q"; then
        if [ "$SESSION_CREATED" -eq 1 ]; then
            echo "[$(date '+%Y-%m-%d %H:%M:%S')] Q window created - attempting qodo startup"
            
            # Check if qodo is available
            if command -v qodo &> /dev/null; then
                echo "[$(date '+%Y-%m-%d %H:%M:%S')] qodo found in PATH - starting login"
                # Use nohup to prevent qodo from blocking tmux
                tmux send-keys -t "$SESSION:Q" "{ q --login && sleep 2 && q --gui; } || echo 'q startup failed'" C-m
            else
                echo "[$(date '+%Y-%m-%d %H:%M:%S')] WARNING: qodo NOT found in PATH"
                echo "[$(date '+%Y-%m-%d %H:%M:%S')] Q window will start as shell only"
                tmux send-keys -t "$SESSION:Q" "echo 'qodo not available - install qodo or add to PATH' && $SHELL" C-m
            fi
        else
            echo "[$(date '+%Y-%m-%d %H:%M:%S')] Q window ready (session already existed; skipping auto qodo commands)"
        fi
    fi

    # prome window: prometheus status/logs
    if tmux list-windows -t "$SESSION" | grep -q "pro"; then
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] Starting prometheus logs in pro window"
        tmux send-keys -t "$SESSION:pro" "sudo journalctl -u prometheus -f" C-m
    fi

    # graf window: grafana status + logs
    if tmux list-windows -t "$SESSION" | grep -q "gra"; then
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] Starting grafana status and logs in gra window"
        tmux send-keys -t "$SESSION:gra" "sudo systemctl status grafana-server && sudo journalctl -u grafana-server -f" C-m
    fi

    # otlp window: start otelcol service and tail its journal
    if tmux list-windows -t "$SESSION" | grep -q "otl"; then
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] Starting otelcol service and logs in otl window"
        tmux send-keys -t "$SESSION:otl" "systemctl --user start otelcol.service && journalctl --user -u otelcol.service -f" C-m
    fi

    # tmux window: tmux-session service status
    if tmux list-windows -t "$SESSION" | grep -q "tmu"; then
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] Starting tmux-session status watch in tmux window"
        tmux send-keys -t "$SESSION:tmu" "watch -n 5 'systemctl --user status tmux-session.service'" C-m
    fi

    # ag window: ag-service status and logs
    if tmux list-windows -t "$SESSION" | grep -q "ag"; then
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] Starting ag-service status and logs in ag window"
        sleep 1
        tmux send-keys -t "$SESSION:ag" "sudo systemctl status ag.service && sudo journalctl -u ag.service -f" C-m
    fi

    # tempo window: tempo status and logs
    if tmux list-windows -t "$SESSION" | grep -q "tem"; then
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] Starting tempo-service status and logs in tem window"
        sleep 1
        tmux send-keys -t "$SESSION:tem" "sudo systemctl status tempo.service && sudo journalctl -u tempo.service -f" C-m
    fi

    # q gui: qodo-gui.service status and logs
    if tmux list-windows -t "$SESSION" | grep -q "q2"; then
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] Starting qodo-gui.service status and logs in q2 window"
        sleep 1
        tmux send-keys -t "$SESSION:q2" "systemctl --user status qodo-gui.service && journalctl --user -u qodo-gui.service -f" C-m
    fi

    # Reload tmux config to apply any formatting
    tmux source-file ~/.tmux.conf 2>/dev/null || true
    
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] Tmux session setup complete v2.1"

} >> "$LOG_FILE" 2>&1