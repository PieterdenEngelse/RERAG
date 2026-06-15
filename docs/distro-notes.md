# RERAG installer — distro support matrix

This file records the verified install status of the RERAG AppImage
installer (`ag-installer-*.AppImage`) across the Linux distros the
project intends to support. Each row is a real install attempt — not
a guess.

## How to verify a distro

Two layers of verification, both producing rows for the matrix below:

### 1. Headless smoke (docker)

`scripts/smoke-test-distros.sh` runs the AppImage's `--version` in
docker containers for Ubuntu 24.04, Debian 12, Fedora 39, and
Arch (rolling). Catches dynamic-linker / glibc-compat issues — fast
(~5 min) but **doesn't exercise the GUI or the install path**.

```bash
# Test the latest released AppImage
scripts/smoke-test-distros.sh

# Test a specific AppImage
scripts/smoke-test-distros.sh /path/to/ag-installer-vX.Y.Z-x86_64.AppImage
```

### 2. Full install (real VM or bare metal)

Spin up a fresh VM (any hypervisor — virt-manager, VirtualBox, GNOME
Boxes, lxc, …). Download the AppImage from the releases page,
`chmod +x`, double-click, walk all six screens through to Done.
**This is the only way to catch GUI / Wayland / installer
end-to-end issues.**

After install, confirm:
- `systemctl --user status ag.service` is `active`
- `curl http://127.0.0.1:3010/health` returns 200
- Dashboard loads and renders styled (no "white window")

## Status matrix

Legend: ✅ verified · 🟡 partial (smoke only, no full install) ·
🔴 known broken · ⏸️ unsupported in v1 · ❔ not yet tested

| Distro | Headless smoke | Full install | Tested version | Date (UTC) | Notes |
|---|---|---|---|---|---|
| **Ubuntu 24.04 LTS** | ✅ | ❔ | v0.4.0 | 2026-06-15 | Build baseline; glibc 2.39 native. Smoke pass on every release via `scripts/smoke-test-distros.sh`. |
| Ubuntu 24.10 / 25.04 | ❔ | ❔ | — | — | Expected to work (newer glibc). |
| **Debian 12 (bookworm)** | 🔴 → ⏸️ | ⏸️ | v0.4.0 | 2026-06-15 | **glibc 2.36 < required 2.39**. Path to support: see "Older-glibc support" below. |
| **Fedora 39** | 🔴 → ⏸️ | ⏸️ | v0.4.0 | 2026-06-15 | **glibc 2.38 < required 2.39**. Same blocker as Debian 12. Fedora 40+ is fine. |
| Fedora 40+ | ❔ | ❔ | — | — | Expected to work (glibc 2.39). Needs verification. |
| **Arch (rolling)** | 🟡 (libxdo) | ❔ | v0.4.0 | 2026-06-15 | glibc OK; **missing `libxdo.so.3`** at runtime — Arch's `xdotool` package doesn't ship it. End users would need to install `libxdo` from AUR or our build needs to bundle libxdo. |
| openSUSE Tumbleweed | ❔ | ❔ | — | — | Rolling — likely glibc 2.39+. Needs verification. |

## The glibc 2.39 baseline

The AppImage requires **glibc 2.39 or newer** at runtime. This comes
from a transitive constraint: the `onnx` Cargo feature pulls the
`ort` crate, which uses prebuilt ONNX Runtime binaries linked against
glibc 2.39. CI builds on `ubuntu-24.04` (glibc 2.39) to match.

Distros affected (cannot run the current AppImage):
- Debian 12 bookworm (glibc 2.36)
- Fedora 39 (glibc 2.38; EOL April 2024)
- Ubuntu 22.04 LTS (glibc 2.35)
- RHEL/Rocky/Alma 9 (glibc 2.34)

### Older-glibc support — path forward (post-v1)

Three options, in increasing order of effort:

1. **Build on `ubuntu-22.04` with `--no-default-features` for onnx.**
   Drops ONNX support (no FastEmbed local embedding); user falls
   back to Ollama embedding via API. Smaller binary, broader compat.
2. **Build ort from source against an older glibc.** Doesn't drop
   features but adds a substantial CI build step (~20-30 min).
3. **Static link via musl.** No glibc dependency at all. Largest
   change — every native dep needs musl-compatible variants. Likely
   blocks webkit2gtk which doesn't musl cleanly.

For v1, the project's position is **glibc 2.39 is the floor**.
Distros with older glibc are listed as `⏸️ unsupported in v1` in the
matrix. Option 1 (drop ONNX) is the cheapest escape hatch if one of
those distros becomes important.

## Other known quirks

### Arch — libxdo

Arch's `xdotool` package no longer ships `libxdo.so.3`. The
installer binary's NEEDED list includes `libxdo.so.3` (Dioxus
desktop → `tao` window crate). Users would see:

```
error while loading shared libraries: libxdo.so.3: cannot open shared object file
```

Workarounds:
- Install `libxdo` from AUR (the canonical shared library).
- Bundle `libxdo.so.3` into the AppImage's `usr/lib/` — adds ~20 KB,
  resolves it for every distro where the system package is missing
  or misnamed. Lowest-friction option for v1.

### Fedora — xattr persistence

The spec (`docs/bin3 §Phase G`) flags that on Fedora, the `chmod +x`
on a downloaded AppImage may not survive certain filesystem operations
unless `user.appimagekit` xattrs are also set. AppImageKit's runtime
handles this transparently on first launch; document any deviation
observed once Fedora 40+ verification happens.

### Wayland vs X11

Dioxus desktop uses webkit2gtk under the hood, which handles
Wayland-vs-X11 transparently in most cases. If the installer window
fails to render under Wayland, capture the journalctl output from the
session and the result of `echo $WAYLAND_DISPLAY $DISPLAY`.

### notify-send

The bash installer (`installers/install-linux.sh`) calls `notify-send`
with `|| true` guards because arg parsing differs across distros. The
Rust installer doesn't use `notify-send` at all (renders progress in
its own window), so this risk doesn't apply.

## Logging a verification

When you verify on a real VM:

1. Update the matrix row above with the status icon, tested AppImage
   version, today's UTC date, and any notes.
2. If it failed: open an issue with:
   - Distro + version (`cat /etc/os-release`)
   - Glibc version (`ldd --version | head -1`)
   - Desktop environment + display server (`echo $XDG_SESSION_TYPE`)
   - Output of `journalctl --user -u ag.service -n 50` if the install
     completed but ag.service didn't start
   - Screenshot or terminal output of where the install broke
