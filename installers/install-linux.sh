#!/usr/bin/env bash
# install-linux.sh — XDG-clean installer for ag (Linux, glibc).
#
# Builds ag, sets up FalkorDB as a native systemd user service, copies
# artifacts to XDG paths, installs three composable systemd user units
# (ag, ag-stack, falkordb), and optionally installs Docker.
#
# Detects existing software and reuses what's already there.
# Verifiable reuses fire silently; non-verifiable ones prompt with a
# "use existing" default. See docs/bin2 for the full design.

set -euo pipefail

# =============================================================================
# Constants and defaults
# =============================================================================

SCRIPT_VERSION="1.0.0"
SCRIPT_PATH="$(realpath "$0")"
SCRIPT_DIR="$(dirname "$SCRIPT_PATH")"
PROJECT_PATH_DEFAULT="$(dirname "$SCRIPT_DIR")"

AG_HOME_DEFAULT="${AG_HOME:-$HOME/.local/share/ag}"
XDG_BIN_DIR="$HOME/.local/bin"
XDG_LIB_DIR="$HOME/.local/lib"
XDG_CONFIG_DIR="$HOME/.config/ag"
SYSTEMD_USER_DIR="$HOME/.config/systemd/user"

# Flag-driven
PROJECT_PATH="$PROJECT_PATH_DEFAULT"
AG_HOME="$AG_HOME_DEFAULT"
BACKEND_PORT=3010
BUILD_MODE="release"
INSTALL_DOCKER=false
NO_SYSTEMD=false
NO_STACK=false
WITH_STACK=""
NO_FRONTEND=false
NO_FALKORDB=false
FALKORDB_PORT=6380
FALKORDB_PASS="agpassword123"
FORCE_COMPOSE=false
NON_INTERACTIVE="${AG_INSTALL_NONINTERACTIVE:-false}"
FORCE_FRESH=false
SKIP_CHECKS=false
VERBOSE=false

# Detection results — populated by detect_*; consumed by step_* and summary.
declare -A DETECT
declare -A DETECT_REUSE_SILENT   # silent verifiable reuses
declare -A DETECT_REUSE_CONFIRM  # prompt outcomes the user confirmed
declare -A DETECT_FRESH          # things being installed fresh
declare -A DETECT_ASSUMPTION     # "⚠ reused with assumption" lines

# Step bookkeeping
declare -a STEP_LIST
STEP_INDEX=0
STEP_TOTAL=0
INSTALL_START=$SECONDS

# Log file (set in setup_log_file)
LOG_FILE=""

# =============================================================================
# Logging
# =============================================================================

c_reset=$'\033[0m'; c_bold=$'\033[1m'
c_red=$'\033[31m'; c_yellow=$'\033[33m'; c_green=$'\033[32m'
c_cyan=$'\033[36m'; c_dim=$'\033[2m'; c_magenta=$'\033[35m'

log_info()  { printf '%s\n' "$*"; }
log_dim()   { printf '%s%s%s\n' "$c_dim" "$*" "$c_reset"; }
log_warn()  { printf '%s⚠ %s%s\n' "$c_yellow" "$*" "$c_reset" >&2; }
log_error() { printf '%s✗ %s%s\n' "$c_red" "$*" "$c_reset" >&2; }
log_ok()    { printf '%s✓ %s%s\n' "$c_green" "$*" "$c_reset"; }

# Step counter [N/TOTAL] ► <name>
step_start() {
    STEP_INDEX=$((STEP_INDEX + 1))
    local name="$1"
    STEP_START_SECONDS=$SECONDS
    STEP_NAME="$name"
    printf '\n%s[%d/%d] ► %s%s\n' "$c_cyan$c_bold" "$STEP_INDEX" "$STEP_TOTAL" "$name" "$c_reset"
}
step_done() {
    local elapsed=$((SECONDS - STEP_START_SECONDS))
    printf '%s[%d/%d] ✓ %s  (%ds)%s\n' "$c_green" "$STEP_INDEX" "$STEP_TOTAL" "$STEP_NAME" "$elapsed" "$c_reset"
}
step_fail() {
    local elapsed=$((SECONDS - STEP_START_SECONDS))
    printf '%s[%d/%d] ✗ %s  (%ds)%s\n' "$c_red" "$STEP_INDEX" "$STEP_TOTAL" "$STEP_NAME" "$elapsed" "$c_reset" >&2
}

# =============================================================================
# Error handling — print last 50 log lines + log path on any failure
# =============================================================================

on_error() {
    local exit_code=$?
    local lineno=${1:-?}
    step_fail 2>/dev/null || true
    log_error "install failed (exit $exit_code at line $lineno)"
    if [[ -n "$LOG_FILE" && -f "$LOG_FILE" ]]; then
        log_error "log: $LOG_FILE"
        log_error "last 50 lines:"
        tail -n 50 "$LOG_FILE" | sed 's/^/    /' >&2
    fi
    exit "$exit_code"
}
trap 'on_error $LINENO' ERR

# =============================================================================
# Flag parsing
# =============================================================================

usage() {
    cat <<EOF
install-linux.sh — XDG-clean installer for ag.

Usage: bash installers/install-linux.sh [OPTIONS]

  --project-path PATH       Repo path (default: $PROJECT_PATH_DEFAULT)
  --prefix PATH             AG_HOME (default: $AG_HOME_DEFAULT)
  --backend-port PORT       BACKEND_PORT (default: 3010)
  --mode release|debug      Cargo build profile (default: release)
  --install-docker          Install Docker via get.docker.com (sudo;
                            no-op if docker is already present).
  --no-systemd              Skip installing all three user units +
                            both ag.service.d drop-ins.
  --no-stack                Skip ag-stack.service install + stack.conf
                            drop-in. ag.service still installs.
  --with-stack=PROFILE      Compose profile written into ag-stack.service
                            (core | observability | "" = all, default "").
  --no-frontend             Skip npm ci + css:build + trunk build.
  --no-falkordb             Skip FalkorDB extraction + falkordb.service +
                            falkordb.conf drop-in.
  --falkordb-port PORT      FalkorDB port (default: 6380).
  --falkordb-password PASS  FalkorDB requirepass (default: agpassword123).
  --force-compose           Ignore active native observability units;
                            install the full compose stack.
  --non-interactive         Take the conservative default on every prompt.
                            Env equivalent: AG_INSTALL_NONINTERACTIVE=1.
                            Does NOT override the --purge confirm.
  --force-fresh             Skip auto-reuse heuristics; overwrite anything
                            detected. Implies "replace" on every prompt.
  --skip-checks             Skip preflight tool + disk checks.
  --verbose                 set -x
  --help, -h                This help.
  --version, -V             $SCRIPT_VERSION

See docs/bin2 for the full design (reuse policy, disk footprint, phasing).
EOF
}

parse_flags() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --project-path)       PROJECT_PATH="$2"; shift ;;
            --prefix)             AG_HOME="$2"; shift ;;
            --backend-port)       BACKEND_PORT="$2"; shift ;;
            --mode)               BUILD_MODE="$2"; shift ;;
            --install-docker)     INSTALL_DOCKER=true ;;
            --no-systemd)         NO_SYSTEMD=true ;;
            --no-stack)           NO_STACK=true ;;
            --with-stack=*)       WITH_STACK="${1#--with-stack=}" ;;
            --with-stack)         WITH_STACK="$2"; shift ;;
            --no-frontend)        NO_FRONTEND=true ;;
            --no-falkordb)        NO_FALKORDB=true ;;
            --falkordb-port)      FALKORDB_PORT="$2"; shift ;;
            --falkordb-password)  FALKORDB_PASS="$2"; shift ;;
            --force-compose)      FORCE_COMPOSE=true ;;
            --non-interactive)    NON_INTERACTIVE=true ;;
            --force-fresh)        FORCE_FRESH=true ;;
            --skip-checks)        SKIP_CHECKS=true ;;
            --verbose|-v)         VERBOSE=true ;;
            --help|-h)            usage; exit 0 ;;
            --version|-V)         printf '%s\n' "$SCRIPT_VERSION"; exit 0 ;;
            *)
                log_error "unknown flag: $1"
                usage
                exit 2
                ;;
        esac
        shift
    done

    $VERBOSE && set -x

    # Validate
    if [[ "$BUILD_MODE" != "release" && "$BUILD_MODE" != "debug" ]]; then
        log_error "--mode must be 'release' or 'debug' (got: $BUILD_MODE)"
        exit 2
    fi
}

# =============================================================================
# Setup
# =============================================================================

setup_log_file() {
    mkdir -p "$AG_HOME/logs"
    local ts; ts="$(date -u +%Y%m%dT%H%M%SZ)"
    LOG_FILE="$AG_HOME/logs/install-$ts.log"
    # Redirect all subsequent stdout/stderr through tee.
    exec > >(tee -a "$LOG_FILE") 2>&1
}

print_banner() {
    log_info ""
    log_info "${c_bold}ag installer v$SCRIPT_VERSION${c_reset}"
    log_info "  project   : $PROJECT_PATH"
    log_info "  AG_HOME   : $AG_HOME"
    log_info "  log       : $LOG_FILE"
    log_info ""
}

# =============================================================================
# Preflight — tools
# =============================================================================

preflight_tools() {
    if $SKIP_CHECKS; then
        log_dim "preflight skipped (--skip-checks)"
        return 0
    fi
    log_dim "preflight: checking required tools"

    local missing=()
    for t in rustc cargo gcc; do
        command -v "$t" >/dev/null 2>&1 || missing+=("$t")
    done

    if ! command -v docker >/dev/null 2>&1; then
        if $INSTALL_DOCKER; then
            log_dim "  docker not present; will install via --install-docker"
        else
            log_error "docker not found on PATH."
            log_error "Either install Docker manually, or re-run with --install-docker."
            exit 1
        fi
    fi

    if ! $NO_FRONTEND; then
        for t in npm trunk; do
            command -v "$t" >/dev/null 2>&1 || missing+=("$t")
        done
    fi

    if [[ ${#missing[@]} -gt 0 ]]; then
        log_error "missing required tools: ${missing[*]}"
        log_error "install them (or pass --no-frontend if frontend tools are missing)"
        exit 1
    fi

    # Soft check: ollama.
    if systemctl --user is-active ollama >/dev/null 2>&1; then
        DETECT[ollama_active]=true
        DETECT_REUSE_SILENT[ollama]="user systemd service, already running"
    else
        DETECT[ollama_active]=false
        log_warn "ollama.service (user) not active — LLM-backed agent modes will return 503 until you start it."
        log_warn "  Set it up manually (NOT via ollama's install.sh — see reference-ollama-setup memory)."
    fi

    log_ok "preflight tools OK"
}

# =============================================================================
# Preflight — disk
# =============================================================================

free_gb() {
    # Available GB on the filesystem containing $1
    df -BG --output=avail "$1" 2>/dev/null | tail -n 1 | tr -dc '0-9'
}

preflight_disk() {
    if $SKIP_CHECKS; then
        return 0
    fi
    local home_gb proj_gb min_gb
    home_gb=$(free_gb "$HOME")
    proj_gb=$(free_gb "$PROJECT_PATH")
    min_gb=$(( home_gb < proj_gb ? home_gb : proj_gb ))

    # Tier-based threshold adjustment
    local hard=10 warn=20
    local target_release="$PROJECT_PATH/target/release/ag"
    local cargo_lock="$PROJECT_PATH/Cargo.lock"
    if [[ -f "$target_release" && -f "$cargo_lock" && "$target_release" -nt "$cargo_lock" ]]; then
        hard=5; warn=10
        DETECT[target_warm]=true
    else
        DETECT[target_warm]=false
    fi

    log_dim "preflight: disk ($min_gb GB free; hard threshold ${hard} GB, warn at ${warn} GB)"

    if (( min_gb < hard )); then
        log_error "< $hard GB free on min(\$HOME, \$PROJECT_PATH). Install would likely fail."
        log_error "Run 'cargo clean' or free up space, or re-run with --skip-checks to override."
        exit 1
    elif (( min_gb < warn )); then
        if $NON_INTERACTIVE; then
            log_error "$min_gb GB free; below warn threshold $warn GB and --non-interactive defaults to refuse."
            log_error "Re-run with --skip-checks to override."
            exit 1
        fi
        log_warn "$min_gb GB free; install peaks ~17 GB on cold-cache."
        local reply
        printf '%sProceed? [y/N]:%s ' "$c_yellow" "$c_reset"
        read -r reply
        [[ "$reply" =~ ^[Yy]$ ]] || { log_info "aborted by user"; exit 0; }
    fi

    log_ok "preflight disk OK"
}

# =============================================================================
# Detection phase — fires before any writes
# =============================================================================

detect_existing_state() {
    log_dim "detecting existing state…"

    # Docker
    if command -v docker >/dev/null 2>&1; then
        DETECT_REUSE_SILENT[docker]="$(docker --version | head -n 1)"
    fi

    # NOTE: ag binary + libtika current-ness is decided in step_install_artifacts
    # (after the build step refreshes target/), not here. Detection-phase results
    # for those two would be stale by the time install_artifacts runs.

    # Existing ag.env — keep, never overwrite
    if [[ -f "$XDG_CONFIG_DIR/ag.env" ]]; then
        DETECT[ag_env_exists]=true
        DETECT_REUSE_SILENT[ag_env]="$XDG_CONFIG_DIR/ag.env exists; preserved"
    else
        DETECT[ag_env_exists]=false
    fi

    # FalkorDB healthy
    if ! $NO_FALKORDB; then
        if systemctl --user is-active falkordb.service >/dev/null 2>&1; then
            local fdb_cli="$AG_HOME/falkordb/redis-cli"
            if [[ -x "$fdb_cli" ]] && "$fdb_cli" -p "$FALKORDB_PORT" -a "$FALKORDB_PASS" ping 2>/dev/null | grep -q PONG; then
                DETECT[falkordb_healthy]=true
                DETECT_REUSE_SILENT[falkordb]="falkordb.service active + PONG on :$FALKORDB_PORT"
            else
                DETECT[falkordb_healthy]=false
            fi
        else
            DETECT[falkordb_healthy]=false
        fi
    fi

    # Compose stack already up (only meaningful if not skipping stack)
    if ! $NO_STACK; then
        if COMPOSE_PROJECT_NAME=ag docker compose ls 2>/dev/null | awk '$1=="ag"{found=1} END{exit !found}'; then
            DETECT[compose_up]=true
            DETECT_REUSE_SILENT[compose]="compose stack already running (project=ag)"
        else
            DETECT[compose_up]=false
        fi
    fi

    # System Redis on 6379 (only if our compose redis isn't the one listening)
    detect_system_redis

    # Native observability
    detect_native_observability

    # Existing ag.service content drift
    detect_ag_service_drift
}

find_newest_libtika() {
    local dirs
    # shellcheck disable=SC2010
    dirs="$(ls -td "$PROJECT_PATH"/target/$BUILD_MODE/build/extractous-*/out/libs 2>/dev/null | head -n 1 || true)"
    [[ -n "$dirs" && -f "$dirs/libtika_native.so" ]] || return 1
    printf '%s\n' "$dirs/libtika_native.so"
}

detect_system_redis() {
    DETECT[system_redis]=false
    # Skip if compose is bringing up ag-redis (the 6379 listener would be ours).
    if [[ "${DETECT[compose_up]:-false}" == "true" ]]; then
        if docker ps --format '{{.Names}}' 2>/dev/null | grep -q '^ag-redis$'; then
            return 0
        fi
    fi
    # Check 6379 directly.
    if command -v redis-cli >/dev/null 2>&1; then
        if redis-cli -p 6379 ping 2>/dev/null | grep -q PONG; then
            DETECT[system_redis]=true
        fi
    fi
}

detect_native_observability() {
    DETECT[native_obs_active]=false
    DETECT[native_obs_units]=""
    if $FORCE_COMPOSE || $NO_STACK; then return 0; fi
    local active=()
    for u in loki tempo otelcol; do
        if systemctl --user is-active "$u" >/dev/null 2>&1; then
            active+=("$u")
        fi
    done
    if [[ ${#active[@]} -gt 0 ]]; then
        DETECT[native_obs_active]=true
        DETECT[native_obs_units]="${active[*]}"
    fi
}

detect_ag_service_drift() {
    DETECT[ag_service_drift]=false
    local installed="$SYSTEMD_USER_DIR/ag.service"
    [[ -f "$installed" ]] || return 0
    # We can't render the template without knowing the substitutions yet,
    # but we can check whether the installed unit has the load-bearing
    # lines from our template. Lightweight heuristic.
    local missing_lines=0
    grep -q "EnvironmentFile=.*ag.env" "$installed" || missing_lines=$((missing_lines + 1))
    grep -q "LD_LIBRARY_PATH=.*lib" "$installed" || missing_lines=$((missing_lines + 1))
    grep -q "ExecStart=.*\.local/bin/ag" "$installed" || missing_lines=$((missing_lines + 1))
    if (( missing_lines > 0 )); then
        DETECT[ag_service_drift]=true
    fi
}

# =============================================================================
# Prompts — non-verifiable reuses
# =============================================================================

prompt_choice() {
    # $1 = prompt text, $2 = default choice letter, rest = options as "K:label"
    # Writes user-facing prompts to /dev/tty so $(...) capture in the caller
    # gets just the final choice, not the whole UI.
    local prompt="$1" default="$2"; shift 2
    if $NON_INTERACTIVE || $FORCE_FRESH; then
        printf '%s\n' "$default"
        return 0
    fi
    {
        printf '\n%s%s%s\n' "$c_yellow" "$prompt" "$c_reset"
        for opt in "$@"; do
            printf '  [%s] %s\n' "${opt%%:*}" "${opt#*:}"
        done
        printf '%sChoice [%s]:%s ' "$c_yellow" "$default" "$c_reset"
    } > /dev/tty
    local reply
    read -r reply < /dev/tty
    [[ -z "$reply" ]] && reply="$default"
    printf '%s\n' "$reply" | tr '[:lower:]' '[:upper:]'
}

run_prompts() {
    log_dim "checking detected state for prompts…"

    # Native observability
    if [[ "${DETECT[native_obs_active]:-false}" == "true" ]]; then
        local active="${DETECT[native_obs_units]}"
        local choice
        choice=$(prompt_choice \
            "Native observability units active: $active. Use them (ag-stack.service skipped)?" \
            "U" \
            "U:Use natives (default) — skip ag-stack.service, leave OTEL_EXPORTER_OTLP_ENDPOINT pointing at native otelcol" \
            "C:Force compose — bring up the full ag-stack anyway" \
            "A:Abort install")
        case "$choice" in
            U)
                DETECT[choice_observability]=natives
                DETECT_REUSE_CONFIRM[observability]="using native $active"
                DETECT_ASSUMPTION[observability]="native $active reused — verify your scrape config covers ag.service /metrics and retention is what you expect"
                NO_STACK=true
                ;;
            C)
                DETECT[choice_observability]=compose
                ;;
            A)  log_info "install aborted at observability prompt."; exit 0 ;;
            *)  log_warn "unrecognized choice '$choice'; using default (natives)"
                DETECT[choice_observability]=natives
                DETECT_REUSE_CONFIRM[observability]="using native $active"
                DETECT_ASSUMPTION[observability]="native $active reused — verify scrape config + retention"
                NO_STACK=true
                ;;
        esac
    fi

    # System Redis
    if [[ "${DETECT[system_redis]:-false}" == "true" ]]; then
        local choice
        choice=$(prompt_choice \
            "System Redis detected on 127.0.0.1:6379. Use it (skip ag-redis from compose)?" \
            "U" \
            "U:Use system Redis (default) — set REDIS_URL=redis://127.0.0.1:6379/ in ag.env" \
            "I:Install ag-redis alongside (compose Redis on :6379 internal, only used if your system Redis goes down)" \
            "A:Abort install")
        case "$choice" in
            U)
                DETECT[choice_redis]=system
                DETECT_REUSE_CONFIRM[redis]="using system Redis at 127.0.0.1:6379"
                DETECT_ASSUMPTION[redis]="system Redis auth not verified — set REDIS_PASSWORD in ag.env if your Redis requires it"
                ;;
            I)
                DETECT[choice_redis]=compose
                ;;
            A)  log_info "install aborted at Redis prompt."; exit 0 ;;
            *)  DETECT[choice_redis]=system
                DETECT_REUSE_CONFIRM[redis]="using system Redis at 127.0.0.1:6379"
                DETECT_ASSUMPTION[redis]="system Redis auth not verified — set REDIS_PASSWORD if needed"
                ;;
        esac
    fi

    # ag.service drift
    if [[ "${DETECT[ag_service_drift]:-false}" == "true" && "$NO_SYSTEMD" == "false" ]]; then
        local choice
        choice=$(prompt_choice \
            "Existing $SYSTEMD_USER_DIR/ag.service differs from the template (likely hand-edited)." \
            "K" \
            "K:Keep existing (default) — skip ag.service rendering" \
            "B:Backup → ag.service.bak-<ts> and replace" \
            "R:Replace without backup")
        DETECT[choice_ag_service]="$choice"
    fi
}

# =============================================================================
# Step planner — compute STEP_TOTAL based on flags + detection
# =============================================================================

plan_steps() {
    STEP_LIST=()
    $INSTALL_DOCKER && ! command -v docker >/dev/null 2>&1 && STEP_LIST+=("install_docker")
    STEP_LIST+=("ensure_xdg" "seed_config" "build_backend")
    $NO_FRONTEND || STEP_LIST+=("build_frontend")
    STEP_LIST+=("install_artifacts")
    $NO_FALKORDB || STEP_LIST+=("falkordb")
    $NO_SYSTEMD  || STEP_LIST+=("systemd")
    STEP_LIST+=("health_check")
    STEP_TOTAL=${#STEP_LIST[@]}
}

# =============================================================================
# Steps
# =============================================================================

step_install_docker() {
    step_start "install Docker"
    if command -v docker >/dev/null 2>&1; then
        log_dim "  docker already present; --install-docker is a no-op"
        step_done; return 0
    fi
    log_info "  fetching https://get.docker.com (sudo will be requested)…"
    sudo true  # prompt for password up front
    curl -fsSL https://get.docker.com -o /tmp/get-docker.sh
    sudo sh /tmp/get-docker.sh
    rm -f /tmp/get-docker.sh
    sudo usermod -aG docker "$USER"
    sudo systemctl enable --now docker
    log_warn "Docker installed. You MUST log out and back in (or run 'newgrp docker')"
    log_warn "before docker commands work in your current shell."
    log_warn "Re-run this installer after that."
    step_done
    exit 0
}

step_ensure_xdg() {
    step_start "ensure XDG tree"
    mkdir -p "$XDG_BIN_DIR" "$XDG_LIB_DIR" "$XDG_CONFIG_DIR"
    mkdir -p "$AG_HOME"/{data,index,db,logs,cache,locks,web,falkordb,falkordb/data}
    mkdir -p "$SYSTEMD_USER_DIR/ag.service.d"
    log_dim "  created/verified ${AG_HOME} tree, $XDG_CONFIG_DIR, $SYSTEMD_USER_DIR/ag.service.d"
    step_done
}

step_seed_config() {
    step_start "seed config"
    local env_target="$XDG_CONFIG_DIR/ag.env"
    local env_source="$PROJECT_PATH/.env.example"
    if [[ ! -f "$env_target" ]]; then
        if [[ ! -f "$env_source" ]]; then
            log_error "$env_source missing — cannot seed ag.env"
            return 1
        fi
        cp "$env_source" "$env_target"
        chmod 0600 "$env_target"
        # Apply flag-driven overrides
        sed -i "s|^BACKEND_PORT=.*|BACKEND_PORT=$BACKEND_PORT|" "$env_target"
        if [[ "${DETECT[choice_redis]:-}" == "system" ]]; then
            sed -i "s|^REDIS_URL=.*|REDIS_URL=redis://127.0.0.1:6379/|" "$env_target"
        fi
        log_ok "  seeded $env_target"
        DETECT_FRESH[ag_env]="$env_target"
    else
        log_dim "  $env_target already exists; not touched"
    fi

    local compose_target="$XDG_CONFIG_DIR/docker-compose.yml"
    local compose_source="$PROJECT_PATH/docker-compose.yml"
    if [[ ! -f "$compose_target" ]]; then
        cp "$compose_source" "$compose_target"
        log_ok "  copied $compose_source → $compose_target"
        DETECT_FRESH[compose_yml]="$compose_target"
    elif ! diff -q "$compose_source" "$compose_target" >/dev/null 2>&1; then
        log_warn "  $compose_target differs from $compose_source"
        log_warn "  keeping your edited version. Run 'diff' on the two to inspect."
        DETECT_ASSUMPTION[compose_yml]="your $compose_target was kept — may not match repo's latest"
    else
        log_dim "  $compose_target up to date"
    fi
    step_done
}

step_build_backend() {
    step_start "build backend (cargo build --$BUILD_MODE)"
    ( cd "$PROJECT_PATH" && cargo build --"$BUILD_MODE" )
    local built="$PROJECT_PATH/target/$BUILD_MODE/ag"
    [[ -f "$built" ]] || { log_error "expected $built but it's missing"; return 1; }
    local size; size=$(du -h "$built" | cut -f1)
    log_dim "  built $built ($size)"
    step_done
}

step_build_frontend() {
    step_start "build frontend (npm + trunk)"
    ( cd "$PROJECT_PATH/frontend/fro" && npm ci --no-audit --no-fund && npm run css:build && trunk build --release )
    [[ -d "$PROJECT_PATH/frontend/fro/dist" ]] || { log_error "frontend/fro/dist missing after build"; return 1; }
    step_done
}

step_install_artifacts() {
    step_start "install artifacts to XDG paths"

    # Binary — copy unless the XDG copy is already newer than the build output
    local built_bin="$PROJECT_PATH/target/$BUILD_MODE/ag"
    if [[ -f "$XDG_BIN_DIR/ag" && -f "$built_bin" \
          && "$XDG_BIN_DIR/ag" -nt "$built_bin" && "$FORCE_FRESH" == "false" ]]; then
        log_dim "  ~/.local/bin/ag is newer than $built_bin; skip copy"
        DETECT_REUSE_SILENT[ag_binary]="~/.local/bin/ag current; skipped"
    else
        install -m 0755 "$built_bin" "$XDG_BIN_DIR/ag"
        log_ok "  installed $XDG_BIN_DIR/ag"
        DETECT_FRESH[ag_binary]="$XDG_BIN_DIR/ag"
    fi

    # libtika
    local libtika; libtika=$(find_newest_libtika || true)
    if [[ -z "$libtika" ]]; then
        log_error "no libtika_native.so under target/$BUILD_MODE/build/extractous-*/out/libs/"
        log_error "the cargo build should produce this; check that extractous is in the dep tree"
        return 1
    fi
    if [[ -f "$XDG_LIB_DIR/libtika_native.so" \
          && "$XDG_LIB_DIR/libtika_native.so" -nt "$libtika" && "$FORCE_FRESH" == "false" ]]; then
        log_dim "  $XDG_LIB_DIR/libtika_native.so is current; skip copy"
        DETECT_REUSE_SILENT[libtika]="$XDG_LIB_DIR/libtika_native.so current; skipped"
    else
        install -m 0644 "$libtika" "$XDG_LIB_DIR/libtika_native.so"
        log_ok "  installed $XDG_LIB_DIR/libtika_native.so (from $(dirname "$libtika"))"
        DETECT_FRESH[libtika]="$XDG_LIB_DIR/libtika_native.so"
    fi

    # Frontend dist
    if ! $NO_FRONTEND; then
        rsync -a --checksum --delete "$PROJECT_PATH/frontend/fro/dist/" "$AG_HOME/web/"
        log_ok "  rsynced frontend/fro/dist/ → $AG_HOME/web/"
        DETECT_FRESH[frontend_dist]="$AG_HOME/web/"
    fi

    # Smoke-test the installed binary (no daemon)
    log_dim "  smoke-test: LD_LIBRARY_PATH=$XDG_LIB_DIR $XDG_BIN_DIR/ag --version"
    if ! LD_LIBRARY_PATH="$XDG_LIB_DIR" "$XDG_BIN_DIR/ag" --version >/dev/null 2>&1; then
        log_error "installed binary failed --version smoke-test"
        log_error "  likely libtika_native.so isn't loading; check $XDG_LIB_DIR"
        return 1
    fi
    log_ok "  binary smoke-test passed"

    step_done
}

step_falkordb() {
    step_start "FalkorDB native service"

    if [[ "${DETECT[falkordb_healthy]:-false}" == "true" && "$FORCE_FRESH" == "false" ]]; then
        log_dim "  falkordb.service already healthy on :$FALKORDB_PORT; skipping extraction + unit install"
        log_dim "  (data dir $AG_HOME/falkordb/data/ left untouched)"
        step_done; return 0
    fi

    # Extract binaries from the falkordb image
    if ! command -v docker >/dev/null 2>&1; then
        log_error "docker not available; cannot extract FalkorDB binaries"
        return 1
    fi
    log_dim "  extracting binaries from falkordb/falkordb:latest…"
    docker pull falkordb/falkordb:latest >/dev/null
    docker rm -f ag-fdb-extract >/dev/null 2>&1 || true
    docker create --name ag-fdb-extract falkordb/falkordb:latest >/dev/null
    docker cp ag-fdb-extract:/usr/local/bin/redis-server          "$AG_HOME/falkordb/" 2>/dev/null
    docker cp ag-fdb-extract:/usr/local/bin/redis-cli             "$AG_HOME/falkordb/" 2>/dev/null
    docker cp ag-fdb-extract:/var/lib/falkordb/bin/falkordb.so    "$AG_HOME/falkordb/" 2>/dev/null
    docker rm ag-fdb-extract >/dev/null
    chmod +x "$AG_HOME/falkordb/redis-server" "$AG_HOME/falkordb/redis-cli"

    # Smoke-test on host before installing unit (catches musl/glibc mismatch).
    if ! "$AG_HOME/falkordb/redis-server" --version >/dev/null 2>&1; then
        log_error "extracted redis-server failed to run on the host (likely musl/glibc mismatch)."
        log_error "See docs/falkordb-native-service.md §2 for fallbacks (distro redis-server + built falkordb.so)."
        return 1
    fi
    log_ok "  binaries extracted and verified"

    # Render the unit
    render_template \
        "$PROJECT_PATH/systemd/falkordb.service.tmpl" \
        "$SYSTEMD_USER_DIR/falkordb.service" \
        "AG_HOME=$AG_HOME" \
        "FDB_PORT=$FALKORDB_PORT" \
        "FDB_PASS=$FALKORDB_PASS"

    systemctl --user daemon-reload
    systemctl --user enable --now falkordb.service

    # Smoke-test the running service
    sleep 1
    if ! "$AG_HOME/falkordb/redis-cli" -p "$FALKORDB_PORT" -a "$FALKORDB_PASS" ping 2>/dev/null | grep -q PONG; then
        log_error "falkordb.service started but redis-cli ping failed"
        return 1
    fi
    log_ok "  falkordb.service active; PONG on :$FALKORDB_PORT"
    DETECT_FRESH[falkordb]="$SYSTEMD_USER_DIR/falkordb.service"
    step_done
}

step_systemd() {
    step_start "systemd user units"

    # ag.service
    local install_ag_service=true
    if [[ "${DETECT[choice_ag_service]:-}" == "K" ]]; then
        log_dim "  keeping existing $SYSTEMD_USER_DIR/ag.service per prompt choice"
        install_ag_service=false
    elif [[ "${DETECT[choice_ag_service]:-}" == "B" ]]; then
        local ts; ts="$(date -u +%Y%m%dT%H%M%SZ)"
        mv "$SYSTEMD_USER_DIR/ag.service" "$SYSTEMD_USER_DIR/ag.service.bak-$ts"
        log_dim "  backed up existing → ag.service.bak-$ts"
    fi
    if $install_ag_service; then
        render_template \
            "$PROJECT_PATH/systemd/ag.service.tmpl" \
            "$SYSTEMD_USER_DIR/ag.service" \
            "AG_BIN=$XDG_BIN_DIR/ag" \
            "AG_HOME=$AG_HOME" \
            "AG_ENV=$XDG_CONFIG_DIR/ag.env" \
            "AG_LIB_DIR=$XDG_LIB_DIR" \
            "BACKEND_PORT=$BACKEND_PORT"
        log_ok "  rendered $SYSTEMD_USER_DIR/ag.service"
        DETECT_FRESH[ag_service]="$SYSTEMD_USER_DIR/ag.service"
    fi

    # ag-stack.service
    if ! $NO_STACK; then
        render_template \
            "$PROJECT_PATH/systemd/ag-stack.service.tmpl" \
            "$SYSTEMD_USER_DIR/ag-stack.service" \
            "COMPOSE_FILE=$XDG_CONFIG_DIR/docker-compose.yml" \
            "COMPOSE_PROFILE=$WITH_STACK"
        log_ok "  rendered $SYSTEMD_USER_DIR/ag-stack.service"
        DETECT_FRESH[ag_stack_service]="$SYSTEMD_USER_DIR/ag-stack.service"
    fi

    # Drop-ins
    if ! $NO_FALKORDB; then
        cp "$PROJECT_PATH/systemd/ag.service.d/falkordb.conf" \
           "$SYSTEMD_USER_DIR/ag.service.d/falkordb.conf"
        log_dim "  installed ag.service.d/falkordb.conf"
    fi
    if ! $NO_STACK; then
        cp "$PROJECT_PATH/systemd/ag.service.d/stack.conf" \
           "$SYSTEMD_USER_DIR/ag.service.d/stack.conf"
        log_dim "  installed ag.service.d/stack.conf"
    fi

    systemctl --user daemon-reload

    if ! $NO_STACK; then
        systemctl --user enable --now ag-stack.service
        log_ok "  ag-stack.service enabled + started"
    fi
    systemctl --user enable --now ag.service
    log_ok "  ag.service enabled + started"

    step_done
}

step_health_check() {
    step_start "post-install health check"
    if $NO_SYSTEMD; then
        log_dim "  --no-systemd was set; nothing to health-check"
        step_done; return 0
    fi
    local url="http://127.0.0.1:$BACKEND_PORT/health"
    log_dim "  polling $url (up to ~20s)…"
    local ok=false
    for i in $(seq 1 10); do
        if curl -fsS --max-time 2 "$url" >/dev/null 2>&1; then ok=true; break; fi
        sleep 2
    done
    if $ok; then
        log_ok "  /health responded 200"
    else
        log_warn "  /health did not respond within ~20s. ag.service may still be starting."
        log_warn "  Check: journalctl --user -u ag.service -n 50"
    fi
    step_done
}

# =============================================================================
# Helpers
# =============================================================================

render_template() {
    local src="$1" dst="$2"; shift 2
    [[ -f "$src" ]] || { log_error "template missing: $src"; return 1; }
    local tmp; tmp="$(mktemp)"
    cp "$src" "$tmp"
    for kv in "$@"; do
        local key="${kv%%=*}" val="${kv#*=}"
        # Use | as sed separator since paths may contain /.
        sed -i "s|{{${key}}}|${val}|g" "$tmp"
    done
    mv "$tmp" "$dst"
    chmod 0644 "$dst"
}

# =============================================================================
# Summary
# =============================================================================

print_summary() {
    local elapsed=$((SECONDS - INSTALL_START))
    log_info ""
    log_info "${c_bold}━━━ Install complete ━━━${c_reset}  (${elapsed}s total)"
    log_info ""
    log_info "Paths:"
    log_info "  binary  : $XDG_BIN_DIR/ag"
    log_info "  libtika : $XDG_LIB_DIR/libtika_native.so"
    log_info "  config  : $XDG_CONFIG_DIR/{ag.env,docker-compose.yml}"
    log_info "  state   : $AG_HOME/"
    log_info "  web     : $AG_HOME/web/"
    log_info "  units   : $SYSTEMD_USER_DIR/{ag,ag-stack,falkordb}.service"
    log_info ""
    log_info "Endpoints:"
    log_info "  ag      : http://127.0.0.1:$BACKEND_PORT (health: /health)"
    log_info "  falkordb: 127.0.0.1:$FALKORDB_PORT (redis-cli, password: $FALKORDB_PASS)"
    if ! $NO_STACK; then
        log_info "  grafana : http://127.0.0.1:3000  (compose stack)"
        log_info "  loki    : http://127.0.0.1:3100"
        log_info "  tempo   : http://127.0.0.1:3200"
        log_info "  otel    : http://127.0.0.1:4318"
        log_info "  prom    : http://127.0.0.1:9090"
    fi
    log_info ""
    log_info "Log file: $LOG_FILE"
    log_info ""

    print_summary_buckets

    log_info ""
    log_info "Next:"
    log_info "  systemctl --user status ag.service"
    log_info "  journalctl --user -u ag.service -f"
    log_info "  curl http://127.0.0.1:$BACKEND_PORT/health"
    if [[ "${DETECT[ollama_active]:-false}" != "true" ]]; then
        log_info ""
        log_info "  Ollama not detected. Set it up manually (NOT via install.sh)"
        log_info "  — see the reference-ollama-setup auto-memory or write"
        log_info "    docs/ollama-native-service.md."
    fi
}

print_summary_buckets() {
    print_bucket "✓ Reused silently" "$c_green" DETECT_REUSE_SILENT
    print_bucket "✓ Reused (confirmed)" "$c_green" DETECT_REUSE_CONFIRM
    print_bucket "+ Installed fresh" "$c_cyan" DETECT_FRESH
    print_bucket "⚠ Reused with assumption" "$c_yellow" DETECT_ASSUMPTION
}

print_bucket() {
    local label="$1" color="$2" array_name="$3"
    declare -n arr="$array_name"
    local n=${#arr[@]}
    if (( n == 0 )); then return 0; fi
    printf '%s%s%s\n' "$color" "$label" "$c_reset"
    for k in "${!arr[@]}"; do
        printf '    %-20s %s\n' "$k" "${arr[$k]}"
    done
}

# =============================================================================
# Main
# =============================================================================

main() {
    parse_flags "$@"
    setup_log_file
    print_banner

    preflight_tools
    preflight_disk

    detect_existing_state
    run_prompts

    plan_steps

    for step in "${STEP_LIST[@]}"; do
        "step_$step"
    done

    print_summary
}

main "$@"
