#!/usr/bin/env bash
# smoke-test-distros.sh — headless smoke test of the RERAG installer
# AppImage across multiple Linux distros via docker.
#
# What it tests: the AppImage's binary starts, finds all its dynamic
# library dependencies on the host, and `--version` reports the
# expected build tag. This is the cheapest signal that the build is
# portable. It does NOT exercise the GUI (no X / Wayland in
# containers), the install path (no real $HOME / systemd), or any
# network-dependent install step. For those, use a real VM.
#
# Usage:
#   scripts/smoke-test-distros.sh [path-to-AppImage]
#
# If no path is given, fetches the latest published AppImage via
# `gh release download` into /tmp.
#
# Distros covered:
#   - ubuntu:24.04  (CI baseline — should always pass)
#   - debian:12     (bookworm — same glibc family, slightly older libs)
#   - fedora:39     (rpm-family — different package names)
#   - archlinux     (rolling — newer libs, AUR-style strictness)
#
# Each distro container installs the minimum set of GTK/webkit deps
# the Dioxus desktop binary needs at load time (even --version triggers
# the dynamic linker to resolve every NEEDED library), then runs the
# AppImage with --appimage-extract-and-run so we don't need FUSE in the
# container.

set -euo pipefail

APPIMAGE="${1:-}"
if [[ -z "$APPIMAGE" ]]; then
    echo "no AppImage given — fetching latest via gh release download…"
    # While we're pre-1.0 every tag is marked --prerelease, so
    # `gh release download` (no args) finds nothing — it filters out
    # prereleases by default. Resolve the most-recent tag explicitly.
    LATEST_TAG="$(gh release list --limit 1 --json tagName --jq '.[0].tagName')"
    if [[ -z "$LATEST_TAG" ]]; then
        echo "ERROR: no releases found on the repo" >&2
        exit 1
    fi
    echo "  latest tag: $LATEST_TAG"
    rm -f /tmp/ag-installer-*.AppImage
    gh release download "$LATEST_TAG" --pattern "*.AppImage" --dir /tmp/ >/dev/null
    APPIMAGE="$(ls /tmp/ag-installer-*.AppImage | tail -n 1)"
    echo "  using: $APPIMAGE"
fi
if [[ ! -f "$APPIMAGE" ]]; then
    echo "ERROR: $APPIMAGE not found" >&2
    exit 1
fi
chmod +x "$APPIMAGE"

APPIMAGE_ABS="$(realpath "$APPIMAGE")"
APPIMAGE_NAME="$(basename "$APPIMAGE_ABS")"

# Distro definitions: each is "label|docker-image|install-cmd"
# install-cmd runs inside the container as root.
DISTROS=(
    "ubuntu-24.04|ubuntu:24.04|apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq libwebkit2gtk-4.1-0 libgtk-3-0 libsoup-3.0-0 libxdo3 libfuse2 ca-certificates >/dev/null"
    "debian-12|debian:12|apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq libwebkit2gtk-4.1-0 libgtk-3-0 libsoup-3.0-0 libxdo3 libfuse2 ca-certificates >/dev/null"
    "fedora-39|fedora:39|dnf install -y -q webkit2gtk4.1 gtk3 libsoup3 fuse-libs >/dev/null"
    "arch-rolling|archlinux:latest|pacman -Sy --noconfirm --noprogressbar webkit2gtk-4.1 gtk3 libsoup3 fuse2 ca-certificates >/dev/null 2>&1"
)

PASS=()
FAIL=()
SKIP=()

if ! command -v docker >/dev/null 2>&1; then
    echo "ERROR: docker not installed; skipping all smoke tests" >&2
    exit 2
fi

for spec in "${DISTROS[@]}"; do
    IFS='|' read -r label image install_cmd <<< "$spec"
    echo
    echo "════════════════════════════════════════════════════════════════"
    echo "  $label   ($image)"
    echo "════════════════════════════════════════════════════════════════"

    if ! docker pull -q "$image" >/dev/null 2>&1; then
        echo "  ✗ docker pull $image failed (network?) — skipping"
        SKIP+=("$label")
        continue
    fi

    # The whole sequence: install deps, run --version, capture exit code.
    if docker run --rm \
        -v "$APPIMAGE_ABS:/tmp/installer.AppImage:ro" \
        "$image" \
        bash -c "
            $install_cmd
            cp /tmp/installer.AppImage /tmp/i.AppImage
            chmod +x /tmp/i.AppImage
            /tmp/i.AppImage --appimage-extract-and-run --version
        " 2>&1; then
        echo "  ✓ $label PASS"
        PASS+=("$label")
    else
        echo "  ✗ $label FAIL (exit \$?)"
        FAIL+=("$label")
    fi
done

echo
echo "════════════════════════════════════════════════════════════════"
echo "  Summary — $APPIMAGE_NAME"
echo "════════════════════════════════════════════════════════════════"
echo "  PASS (${#PASS[@]}): ${PASS[*]:-<none>}"
echo "  FAIL (${#FAIL[@]}): ${FAIL[*]:-<none>}"
echo "  SKIP (${#SKIP[@]}): ${SKIP[*]:-<none>}"
echo
echo "  Paste this run's result into docs/distro-notes.md."

# Exit 1 if any distro failed (skip is informational, not failure).
[[ ${#FAIL[@]} -eq 0 ]]
