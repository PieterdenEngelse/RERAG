#!/bin/bash
# ~/ag/tmux_startup.sh v1.2.1
# Initialize tmux sessions "ba" and "fo" with standard windows
# Handles all tmux session/window management and auto-attach
set -euo pipefail

LOG_FILE="$HOME/.tmux_startup.log"
MENU_DIR="/tools/tmux_menu"
MENU_MIN_LINES=${MENU_MIN_LINES:-7}

{
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] Starting tmux session setup"
    
    SESSION="ba"
    AG_HOME="$HOME/ag"
    SESSION_CREATED=0

    # Safe tmux wrapper - logs failures but doesn't kill script
    tmux_safe() {
        local desc="$1"
        shift
        if ! tmux "$@" 2>/dev/null; then
            echo "[$(date '+%Y-%m-%d %H:%M:%S')] WARNING: $desc failed: tmux $*"
            return 1
        fi
        return 0
    }

    # Helper function to create a window only if it doesn't exist
    create_window_if_missing() {
        local window_name="$1"
        local window_path="$2"
        if tmux list-windows -t "$SESSION" -F "#{window_name}" 2>/dev/null | grep -q "^${window_name}$"; then
            echo "[$(date '+%Y-%m-%d %H:%M:%S')] Window $window_name already exists"
            return 0
        else
            echo "[$(date '+%Y-%m-%d %H:%M:%S')] Creating window $window_name"
            tmux_safe "create window $window_name" new-window -t "$SESSION:" -n "$window_name" -c "$window_path" -d || true
        fi
    }

    # Helper function to setup a service monitoring window with split panes and menu
    setup_service_window() {
        local window_name="$1"
        local service_name="$2"
        local use_sudo="$3"
        local menu_script="$4"
        
        # Check if window exists
        if ! tmux list-windows -t "$SESSION" -F "#{window_name}" 2>/dev/null | grep -q "^${window_name}$"; then
            echo "[$(date '+%Y-%m-%d %H:%M:%S')] Window $window_name does not exist, skipping setup"
            return 0
        fi
        
        # Check if already has multiple panes (already split)
        local pane_count
        pane_count=$(tmux list-panes -t "$SESSION:$window_name" 2>/dev/null | wc -l)
        if [ "$pane_count" -gt 1 ]; then
            echo "[$(date '+%Y-%m-%d %H:%M:%S')] Window $window_name already has $pane_count panes, skipping split"
            return 0
        fi
        
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] Setting up $window_name window with split panes"
        
        # Split window: default split
        tmux_safe "split $window_name" split-window -v -t "$SESSION:$window_name" || return 0

        # Resize so pane 0 keeps minimal height for menu
        if [ "$MENU_MIN_LINES" -gt 0 ]; then
            tmux_safe "resize $window_name" resize-pane -t "$SESSION:$window_name.0" -y "$MENU_MIN_LINES" || true
        fi
        
        # Set pane titles
        tmux_safe "title $window_name.0" select-pane -t "$SESSION:$window_name.0" -T "Menu" || true
        tmux_safe "title $window_name.1" select-pane -t "$SESSION:$window_name.1" -T "Status" || true
        
        # Pane 0 = top (menu), Pane 1 = bottom (watch status)
        tmux_safe "menu cmd $window_name" send-keys -t "$SESSION:$window_name.0" "$MENU_DIR/$menu_script" C-m || true
        
        if [ "$use_sudo" = "sudo" ]; then
            tmux_safe "status cmd $window_name" send-keys -t "$SESSION:$window_name.1" "watch -n 5 'sudo systemctl status $service_name'" C-m || true
        else
            tmux_safe "status cmd $window_name" send-keys -t "$SESSION:$window_name.1" "watch -n 5 'systemctl --user status $service_name'" C-m || true
        fi

        # Ensure menu pane has focus so keypresses land in the menu immediately
        tmux_safe "focus menu $window_name" select-pane -t "$SESSION:$window_name.0" || true
    }

    # Helper function to create a window in fo session
    create_fo_window_if_missing() {
        local window_name="$1"
        local window_path="$2"
        if tmux list-windows -t "fo" -F "#{window_name}" 2>/dev/null | grep -q "^${window_name}$"; then
            echo "[$(date '+%Y-%m-%d %H:%M:%S')] Window fo:$window_name already exists"
            return 0
        else
            echo "[$(date '+%Y-%m-%d %H:%M:%S')] Creating window fo:$window_name"
            tmux_safe "create fo:$window_name" new-window -t "fo:" -n "$window_name" -c "$window_path" -d || true
        fi
    }

    # Helper function to setup service window in fo session
    setup_fo_service_window() {
        local window_name="$1"
        local service_name="$2"
        local use_sudo="$3"
        local menu_script="$4"
        
        if ! tmux list-windows -t "fo" -F "#{window_name}" 2>/dev/null | grep -q "^${window_name}$"; then
            echo "[$(date '+%Y-%m-%d %H:%M:%S')] Window fo:$window_name does not exist, skipping setup"
            return 0
        fi
        
        local pane_count
        pane_count=$(tmux list-panes -t "fo:$window_name" 2>/dev/null | wc -l)
        if [ "$pane_count" -gt 1 ]; then
            echo "[$(date '+%Y-%m-%d %H:%M:%S')] Window fo:$window_name already has $pane_count panes, skipping split"
            return 0
        fi
        
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] Setting up fo:$window_name window with split panes"
        
        # Split window
        tmux_safe "split fo:$window_name" split-window -v -t "fo:$window_name" || return 0

        # Resize pane 0 to menu height
        if [ "$MENU_MIN_LINES" -gt 0 ]; then
            tmux_safe "resize fo:$window_name" resize-pane -t "fo:$window_name.0" -y "$MENU_MIN_LINES" || true
        fi
        
        tmux_safe "title fo:$window_name.0" select-pane -t "fo:$window_name.0" -T "Menu" || true
        tmux_safe "title fo:$window_name.1" select-pane -t "fo:$window_name.1" -T "Status" || true
        
        tmux_safe "menu cmd fo:$window_name" send-keys -t "fo:$window_name.0" "$MENU_DIR/$menu_script" C-m || true
        
        if [ "$use_sudo" = "sudo" ]; then
            tmux_safe "status cmd fo:$window_name" send-keys -t "fo:$window_name.1" "watch -n 5 'sudo systemctl status $service_name'" C-m || true
        else
            tmux_safe "status cmd fo:$window_name" send-keys -t "fo:$window_name.1" "watch -n 5 'systemctl --user status $service_name'" C-m || true
        fi

        tmux_safe "focus menu fo:$window_name" select-pane -t "fo:$window_name.0" || true
    }

    # CRITICAL: Create "ba" session - fail here kills script (intentional)
    if ! tmux has-session -t "$SESSION" 2>/dev/null; then
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] Creating new tmux session $SESSION"
        tmux new-session -s "$SESSION" -d -c "$AG_HOME" -n "Q"
        SESSION_CREATED=1
    fi

    # CRITICAL: Create "fo" session - fail here kills script (intentional)
    if ! tmux has-session -t "fo" 2>/dev/null; then
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] Creating new tmux session fo"
        tmux new-session -d -s "fo" -c "$HOME/ag/frontend/fro" -n "dl"
    fi

    # Set convenient session hotkeys (Alt+1 => fo, Alt+2 => ba)
    tmux_safe "bind alt1" bind-key -n M-1 switch-client -t fo || true
    tmux_safe "bind alt2" bind-key -n M-2 switch-client -t ba || true

    # Create named windows for "ba" session (non-critical)
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] Creating windows..."
    create_window_if_missing "pr" "$AG_HOME"
    create_window_if_missing "gr" "$AG_HOME"
    create_window_if_missing "ot" "$AG_HOME"
    create_window_if_missing "tm" "$HOME"
    create_window_if_missing "ag" "$HOME"
    create_window_if_missing "te" "$HOME"
    create_window_if_missing "lo" "$HOME"
    create_window_if_missing "ve" "$HOME"
    create_window_if_missing "al" "$HOME"
    create_window_if_missing "q2" "$HOME"
    create_fo_window_if_missing "dx" "$HOME/ag/frontend/fro"
    create_fo_window_if_missing "dl" "$HOME/ag/frontend/fro"
    create_fo_window_if_missing "cs" "$HOME/ag/frontend/fro"

    # Send commands to specific windows
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] Setting up window commands"

    # Q window: auto-run qodo login/gui only once per new tmux session
    if tmux list-windows -t "$SESSION" 2>/dev/null | grep -q "Q"; then
        if [ "$SESSION_CREATED" -eq 1 ]; then
            echo "[$(date '+%Y-%m-%d %H:%M:%S')] Starting qodo in Q window (one-time for this session)"
            tmux_safe "q --login" send-keys -t "$SESSION:Q" "q --login" C-m || true
            sleep 1
            tmux_safe "q --gui" send-keys -t "$SESSION:Q" "q --gui" C-m || true
        else
            echo "[$(date '+%Y-%m-%d %H:%M:%S')] Q window ready (session already existed; skipping auto qodo commands)"
        fi
    fi

    # Setup service monitoring windows with split panes for "ba" session
    # Format: window_name, service_name, sudo|user, menu_script
    setup_service_window "pr" "prometheus"           "sudo" "prometheus_menu.sh"
    setup_service_window "gr" "grafana-server"       "sudo" "grafana_menu.sh"
    setup_service_window "ot" "otelcol.service"      "user" "otelcol_menu.sh"
    setup_service_window "ag" "ag.service"           "sudo" "ag_menu.sh"
    setup_service_window "te" "tempo.service"        "sudo" "tempo_menu.sh"
    setup_service_window "lo" "loki.service"         "user" "loki_menu.sh"
    setup_service_window "ve" "vector.service"       "user" "vector_menu.sh"
    setup_service_window "al" "alertmanager.service" "user" "alertmanager_menu.sh"
    setup_service_window "q2" "qodo-gui.service"     "user" "qodo_menu.sh"

    # tm window: tmux-session service status (single pane, just watch)
    if tmux list-windows -t "$SESSION" 2>/dev/null | grep -q "tm"; then
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] Starting tmux-session status watch in tm window"
        tmux_safe "tm status" send-keys -t "$SESSION:tm" "watch -n 5 'systemctl --user status tmux-session.service'" C-m || true
    fi

    # Setup service monitoring windows for "fo" session
    setup_fo_service_window "dx" "dioxus.service" "sudo" "dioxus_menu.sh"

    # Reload tmux config to apply any formatting
    tmux source-file ~/.tmux.conf 2>/dev/null || true
    
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] Tmux session setup complete"

} >> "$LOG_FILE" 2>&1

# Attach to session if not already inside tmux (outside logging block)
if [ -z "${TMUX:-}" ] && [ -t 0 ]; then
    exec tmux attach -t ba
fi