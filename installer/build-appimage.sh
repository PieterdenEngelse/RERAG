#!/usr/bin/env bash
# build-appimage.sh — assemble the AppDir/ tree and call appimagetool.
#
# Run by .github/workflows/release.yml after cargo+trunk+falkordb-extract
# have produced the artifacts this script bundles. Can also run locally
# for dev iteration: build the prereqs by hand, then invoke this script.
#
# See docs/bin3 §Phase A for the AppImage bundle layout.

set -euo pipefail

# -----------------------------------------------------------------------------
# Inputs (overridable via env)
# -----------------------------------------------------------------------------

REPO_ROOT="${REPO_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
TARGET_DIR="${TARGET_DIR:-${REPO_ROOT}/target/release}"
FALKORDB_STAGE="${FALKORDB_STAGE:-${REPO_ROOT}/installer/stage/falkordb}"
APPDIR="${APPDIR:-${REPO_ROOT}/installer/AppDir}"
VERSION="${VERSION:-$(grep -E '^version = ' "${REPO_ROOT}/installer/Cargo.toml" | head -1 | cut -d'"' -f2)}"
ARCH="${ARCH:-x86_64}"
OUTPUT="${OUTPUT:-${REPO_ROOT}/ag-installer-v${VERSION}-${ARCH}.AppImage}"
APPIMAGETOOL="${APPIMAGETOOL:-$(command -v appimagetool || true)}"

c_dim=$'\033[2m'; c_red=$'\033[31m'; c_green=$'\033[32m'; c_reset=$'\033[0m'
log()  { printf '%s%s%s\n' "$c_dim" "$*" "$c_reset"; }
ok()   { printf '%s✓ %s%s\n' "$c_green" "$*" "$c_reset"; }
fail() { printf '%s✗ %s%s\n' "$c_red" "$*" "$c_reset" >&2; exit 1; }

# -----------------------------------------------------------------------------
# Preflight
# -----------------------------------------------------------------------------

require_file() {
    [[ -f "$1" ]] || fail "missing input: $1 ($2)"
}
require_dir() {
    [[ -d "$1" ]] || fail "missing input directory: $1 ($2)"
}

log "build-appimage.sh — assembling AppDir/ for ag-installer v${VERSION}"
[[ -n "$APPIMAGETOOL" ]] || fail "appimagetool not on PATH. Download from github.com/AppImage/AppImageKit/releases"

require_file "${TARGET_DIR}/ag-installer" "Phase A: cargo build --release -p ag-installer"
require_file "${TARGET_DIR}/ag"           "backend: cargo build --release -p ag"
# Frontend dist + libtika + FalkorDB binaries are warnings-only at this phase
# because Phase A's foundation goal is just to produce an AppImage; bundling
# the rest of the runtime artifacts is Phase D's job.

# -----------------------------------------------------------------------------
# Assemble AppDir
# -----------------------------------------------------------------------------

rm -rf "$APPDIR"
mkdir -p "$APPDIR"/{usr/bin,usr/lib,usr/share/ag,usr/share/applications,usr/share/icons/hicolor/512x512/apps}

# Top-level shims appimagetool requires.
cat > "$APPDIR/AppRun" <<'APPRUN_EOF'
#!/bin/bash
# AppRun shim: set up env and exec the installer binary.
SELF_DIR="$(dirname "$(readlink -f "$0")")"

# Pre-flight: glibc 2.39 required by the bundled ort/onnxruntime. Without
# this check, users on older glibc just see "GLIBC_2.39 not found" from
# the dynamic linker (or nothing at all when double-clicked) — the GUI
# never opens. Fail politely with terminal output + a GUI dialog when
# zenity/kdialog are available.
GLIBC="$(ldd --version 2>/dev/null | head -1 | grep -oE '[0-9]+\.[0-9]+' | head -1)"
if [[ -n "$GLIBC" ]] && awk "BEGIN {exit !(${GLIBC} < 2.39)}"; then
    MSG="RERAG installer requires glibc 2.39+ (you have ${GLIBC}).

Supported AppImage targets: Ubuntu 24.04+, Fedora 40+, Arch, openSUSE Tumbleweed.

Older distros (Debian 12, Fedora 39, Ubuntu 22.04, RHEL/Rocky 9):
build from source instead — clone github.com/PieterdenEngelse/RERAG
and run installers/install-linux.sh. ag compiles against your system's
glibc with no AppImage involved."
    echo "$MSG" >&2
    if command -v zenity >/dev/null 2>&1; then
        zenity --error --width=520 --title="RERAG installer" --text="$MSG" 2>/dev/null
    elif command -v kdialog >/dev/null 2>&1; then
        kdialog --error "$MSG" --title "RERAG installer" 2>/dev/null
    fi
    exit 1
fi

export PATH="${SELF_DIR}/usr/bin:${PATH}"
# libtika lives under usr/lib/ when bundled (Phase D); harmless if absent.
export LD_LIBRARY_PATH="${SELF_DIR}/usr/lib:${LD_LIBRARY_PATH:-}"
# Tell the installer where its bundled assets live so it doesn't have to guess.
export AG_INSTALLER_BUNDLE_ROOT="${SELF_DIR}/usr/share/ag"
exec "${SELF_DIR}/usr/bin/ag-installer" "$@"
APPRUN_EOF
chmod +x "$APPDIR/AppRun"

# The .desktop file and icon need to be at AppDir root AND under usr/share/.
cp "${REPO_ROOT}/installer/assets/ag-installer.desktop"  "$APPDIR/"
cp "${REPO_ROOT}/installer/assets/ag-installer.desktop"  "$APPDIR/usr/share/applications/"
cp "${REPO_ROOT}/installer/assets/icon.png"              "$APPDIR/ag-installer.png"
cp "${REPO_ROOT}/installer/assets/icon.png"              "$APPDIR/usr/share/icons/hicolor/512x512/apps/ag-installer.png"

# Phase A core binary.
cp "${TARGET_DIR}/ag-installer" "$APPDIR/usr/bin/"
chmod 0755 "$APPDIR/usr/bin/ag-installer"

# Pre-built ag backend binary (bundled at AppImage build time — see bin3).
cp "${TARGET_DIR}/ag" "$APPDIR/usr/bin/"
chmod 0755 "$APPDIR/usr/bin/ag"

# Optional bundles (Phase D fills these out; Phase A doesn't gate on them).
if [[ -f "${TARGET_DIR}/build/extractous-"*"/out/libs/libtika_native.so" ]]; then
    LIBTIKA="$(ls -td "${TARGET_DIR}/build/extractous-"*"/out/libs/libtika_native.so" 2>/dev/null | head -n 1)"
    cp "$LIBTIKA" "$APPDIR/usr/lib/"
    ok "bundled libtika_native.so ($(du -h "$LIBTIKA" | cut -f1))"
else
    log "  libtika_native.so not found — skipping (Phase D will require it)"
fi

# Phase G fix: bundle libxdo.so.3.
# The installer binary's NEEDED list includes libxdo.so.3 (Dioxus desktop →
# tao window crate). Arch's `xdotool` package no longer ships this library,
# and a few other distros are inconsistent. Bundling it (~20 KB) makes the
# installer self-sufficient on every target with glibc 2.39+. See
# docs/distro-notes.md §Arch — libxdo.
LIBXDO="$(ldconfig -p 2>/dev/null | awk '/libxdo\.so\.3/ {print $4; exit}')"
if [[ -n "$LIBXDO" && -f "$LIBXDO" ]]; then
    # Resolve to the real file (ldconfig may report a symlink path).
    LIBXDO_REAL="$(readlink -f "$LIBXDO")"
    cp "$LIBXDO_REAL" "$APPDIR/usr/lib/libxdo.so.3"
    ok "bundled libxdo.so.3 from $LIBXDO_REAL ($(du -h "$LIBXDO_REAL" | cut -f1))"
else
    log "  libxdo.so.3 not on the build host — skipping (Arch users will see the missing-lib error)"
fi

# dx build --platform web outputs to target/dx/<pkg>/release/web/public/,
# not frontend/fro/dist/ (which was trunk's output path). The dx output
# includes hashed asset filenames that the runtime asset!() injection
# expects.
FRONTEND_DIST="${REPO_ROOT}/target/dx/fro/release/web/public"
if [[ -d "$FRONTEND_DIST" ]]; then
    mkdir -p "$APPDIR/usr/share/ag/web"
    cp -r "$FRONTEND_DIST"/* "$APPDIR/usr/share/ag/web/"
    ok "bundled $FRONTEND_DIST → usr/share/ag/web/"
else
    log "  $FRONTEND_DIST not found — skipping"
fi

if [[ -d "${FALKORDB_STAGE}" ]]; then
    mkdir -p "$APPDIR/usr/share/ag/falkordb"
    cp "${FALKORDB_STAGE}"/* "$APPDIR/usr/share/ag/falkordb/"
    ok "bundled FalkorDB binaries from staging"
else
    log "  FalkorDB staging dir not found — skipping (Phase D will require it)"
fi

# Compose file + env example + systemd templates — small text files, always bundle.
[[ -f "${REPO_ROOT}/docker-compose.yml" ]] && \
    cp "${REPO_ROOT}/docker-compose.yml" "$APPDIR/usr/share/ag/"
[[ -f "${REPO_ROOT}/.env.example" ]] && \
    cp "${REPO_ROOT}/.env.example" "$APPDIR/usr/share/ag/"
if [[ -d "${REPO_ROOT}/systemd" ]]; then
    mkdir -p "$APPDIR/usr/share/ag/systemd"
    cp -r "${REPO_ROOT}/systemd/"*.tmpl "$APPDIR/usr/share/ag/systemd/" 2>/dev/null || true
    cp -r "${REPO_ROOT}/systemd/ag.service.d" "$APPDIR/usr/share/ag/systemd/" 2>/dev/null || true
fi

ok "AppDir/ assembled at $APPDIR"

# -----------------------------------------------------------------------------
# appimagetool
# -----------------------------------------------------------------------------

log "running appimagetool…"
ARCH="$ARCH" "$APPIMAGETOOL" "$APPDIR" "$OUTPUT" 2>&1 | grep -vE "^(WARNING|appimagetool|gpg)" || true
[[ -f "$OUTPUT" ]] || fail "appimagetool did not produce $OUTPUT"
chmod +x "$OUTPUT"

# -----------------------------------------------------------------------------
# Report
# -----------------------------------------------------------------------------

SIZE="$(du -h "$OUTPUT" | cut -f1)"
SHA="$(sha256sum "$OUTPUT" | cut -d' ' -f1)"
ok "produced $OUTPUT ($SIZE)"
echo "$SHA  $(basename "$OUTPUT")" > "${OUTPUT}.sha256"
log "sha256: $SHA"
log "sha256 file: ${OUTPUT}.sha256"
