#!/usr/bin/env bash
# uninstall-linux.sh — symmetric counterpart to install-linux.sh.
#
# Removes the three ag user systemd units, the drop-ins, the XDG
# artifacts (~/.local/bin/ag, ~/.local/lib/libtika_native.so), and
# optionally the runtime state under ~/.local/share/ag and ~/.config/ag
# (behind --purge, with confirm).
#
# Does NOT uninstall Docker even if install-linux.sh --install-docker
# was used to install it. Does NOT touch ~/.cargo, target/, or
# frontend/fro/node_modules (those are dev-tool state, not ag state).
#
# See docs/bin2 for the full design.

set -euo pipefail

# -----------------------------------------------------------------------------
# Constants
# -----------------------------------------------------------------------------

SCRIPT_VERSION="1.0.0"
AG_HOME_DEFAULT="${AG_HOME:-$HOME/.local/share/ag}"
SYSTEMD_USER_DIR="$HOME/.config/systemd/user"
XDG_BIN="$HOME/.local/bin/ag"
XDG_LIB="$HOME/.local/lib/libtika_native.so"
XDG_CONFIG_DIR="$HOME/.config/ag"

# Flag-driven
PURGE=false
ASSUME_YES=false
COMPOSE_DOWN=true
QUIET=false

# -----------------------------------------------------------------------------
# Logging
# -----------------------------------------------------------------------------

c_reset=$'\033[0m'
c_red=$'\033[31m'
c_yellow=$'\033[33m'
c_green=$'\033[32m'
c_cyan=$'\033[36m'
c_dim=$'\033[2m'

log_info()  { $QUIET && return 0; printf '%s\n' "$*"; }
log_step()  { $QUIET && return 0; printf '%s► %s%s\n' "$c_cyan" "$*" "$c_reset"; }
log_ok()    { $QUIET && return 0; printf '%s✓ %s%s\n' "$c_green" "$*" "$c_reset"; }
log_warn()  { printf '%s⚠ %s%s\n' "$c_yellow" "$*" "$c_reset" >&2; }
log_error() { printf '%s✗ %s%s\n' "$c_red" "$*" "$c_reset" >&2; }

# -----------------------------------------------------------------------------
# Flag parsing
# -----------------------------------------------------------------------------

usage() {
    cat <<EOF
uninstall-linux.sh — remove the ag systemd units and XDG artifacts.

Usage: bash installers/uninstall-linux.sh [OPTIONS]

  --purge            Also delete \$AG_HOME (~/.local/share/ag — includes
                     FalkorDB data, Tantivy index, app logs) and
                     ~/.config/ag (env file + compose file). Requires
                     y/N confirm unless --yes is also passed.
  --yes              Skip the --purge confirm prompt. No effect without
                     --purge.
  --no-compose-down  Skip 'docker compose down' for the ag-stack.
                     Default is to bring the compose stack down.
  --quiet            Reduce non-error output.
  --help             This help.
  --version          $SCRIPT_VERSION

What gets removed (default — no --purge):

  ~/.config/systemd/user/ag.service
  ~/.config/systemd/user/ag.service.d/{falkordb,stack}.conf
  ~/.config/systemd/user/ag-stack.service
  ~/.config/systemd/user/falkordb.service
  ~/.local/bin/ag
  ~/.local/lib/libtika_native.so
  ~/.local/share/ag/web/  (frontend dist)

What --purge ALSO removes (irreversible):

  ~/.local/share/ag/       (FalkorDB data, Tantivy index, app logs, caches)
  ~/.config/ag/            (ag.env, docker-compose.yml)
EOF
}

parse_flags() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --purge)            PURGE=true ;;
            --yes|-y)           ASSUME_YES=true ;;
            --no-compose-down)  COMPOSE_DOWN=false ;;
            --quiet|-q)         QUIET=true ;;
            --help|-h)          usage; exit 0 ;;
            --version|-V)       printf '%s\n' "$SCRIPT_VERSION"; exit 0 ;;
            *)
                log_error "unknown flag: $1"
                usage
                exit 2
                ;;
        esac
        shift
    done
}

# -----------------------------------------------------------------------------
# Steps
# -----------------------------------------------------------------------------

stop_units() {
    log_step "stop + disable systemd user units"
    local units=(ag.service ag-stack.service falkordb.service)
    for u in "${units[@]}"; do
        if systemctl --user list-unit-files --no-legend 2>/dev/null | grep -q "^$u"; then
            systemctl --user disable --now "$u" 2>/dev/null || log_warn "disable --now $u failed (continuing)"
        fi
    done
    log_ok "units stopped + disabled"
}

remove_unit_files() {
    log_step "remove unit files + drop-ins"
    rm -f "$SYSTEMD_USER_DIR/ag.service"
    rm -f "$SYSTEMD_USER_DIR/ag-stack.service"
    rm -f "$SYSTEMD_USER_DIR/falkordb.service"
    rm -f "$SYSTEMD_USER_DIR/ag.service.d/falkordb.conf"
    rm -f "$SYSTEMD_USER_DIR/ag.service.d/stack.conf"
    # Remove the drop-in dir only if empty (user may have other drop-ins).
    rmdir "$SYSTEMD_USER_DIR/ag.service.d" 2>/dev/null || true
    systemctl --user daemon-reload 2>/dev/null || true
    log_ok "unit files removed"
}

compose_down() {
    if ! $COMPOSE_DOWN; then
        log_info "${c_dim}skipping compose down (--no-compose-down)${c_reset}"
        return 0
    fi
    if ! command -v docker >/dev/null 2>&1; then
        log_info "${c_dim}docker not on PATH; skipping compose down${c_reset}"
        return 0
    fi
    local compose_file="$XDG_CONFIG_DIR/docker-compose.yml"
    if [[ ! -f "$compose_file" ]]; then
        log_info "${c_dim}no $compose_file; skipping compose down${c_reset}"
        return 0
    fi
    log_step "bring ag compose stack down"
    COMPOSE_PROJECT_NAME=ag docker compose -f "$compose_file" down 2>/dev/null || \
        log_warn "compose down reported errors (containers may already be stopped)"
    log_ok "compose stack down"
}

remove_artifacts() {
    log_step "remove XDG artifacts"
    rm -f "$XDG_BIN"
    rm -f "$XDG_LIB"
    # Frontend dist gets removed in non-purge too — it's pure derived state.
    rm -rf "$AG_HOME_DEFAULT/web"
    log_ok "artifacts removed (~/.local/bin/ag, ~/.local/lib/libtika_native.so, web/)"
}

purge_runtime_state() {
    if ! $PURGE; then
        log_info "${c_dim}keeping \$AG_HOME and ~/.config/ag (use --purge to delete)${c_reset}"
        return 0
    fi
    log_warn "--purge will DELETE:"
    log_warn "    $AG_HOME_DEFAULT"
    log_warn "    $XDG_CONFIG_DIR"
    log_warn "This includes FalkorDB data, Tantivy index, app logs, ag.env."
    log_warn "This is IRREVERSIBLE."
    if ! $ASSUME_YES; then
        local reply=""
        printf '%sProceed? Type "yes" to confirm:%s ' "$c_red" "$c_reset"
        read -r reply
        if [[ "$reply" != "yes" ]]; then
            log_info "purge aborted; keeping data."
            return 0
        fi
    fi
    log_step "purging runtime state"
    rm -rf "$AG_HOME_DEFAULT"
    rm -rf "$XDG_CONFIG_DIR"
    log_ok "purged $AG_HOME_DEFAULT and $XDG_CONFIG_DIR"
}

# -----------------------------------------------------------------------------
# Main
# -----------------------------------------------------------------------------

main() {
    parse_flags "$@"
    log_info "ag uninstall (script v$SCRIPT_VERSION)"
    log_info "  AG_HOME : $AG_HOME_DEFAULT"
    log_info "  units   : $SYSTEMD_USER_DIR"
    log_info ""

    stop_units
    compose_down
    remove_unit_files
    remove_artifacts
    purge_runtime_state

    log_info ""
    log_ok "uninstall complete"
    if ! $PURGE; then
        log_info ""
        log_info "Runtime state preserved at:"
        log_info "    $AG_HOME_DEFAULT"
        log_info "    $XDG_CONFIG_DIR"
        log_info "Re-running install-linux.sh will pick these up (warm-cache install)."
    fi
}

main "$@"
