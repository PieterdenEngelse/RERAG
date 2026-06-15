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
| Ubuntu 24.04 LTS | ✅ (CI baseline) | ❔ | v0.4.0 | 2026-06-15 | CI builds + smokes on every release tag. Glibc 2.39 baseline. |
| Debian 12 (bookworm) | ❔ | ❔ | — | — | Same glibc family as Ubuntu; expect pass once smoked. |
| Fedora 39 | ❔ | ❔ | — | — | Different package names (`webkit2gtk4.1`, `fuse-libs`). Spec calls out xattr quirk — verify `chmod +x` survives a logout/login. |
| Arch (rolling) | ❔ | ❔ | — | — | Newer libs than the CI baseline; failures here often mean the AppImage's bundled libs need a refresh. |
| openSUSE Tumbleweed | ❔ | ❔ | — | — | Optional v1 target. |

## Known quirks

### Fedora — xattr persistence

The spec (`docs/bin3 §Phase G`) flags that on Fedora, the `chmod +x`
on a downloaded AppImage may not survive certain filesystem operations
unless `user.appimagekit` xattrs are also set. AppImageKit's runtime
handles this transparently on first launch; document any deviation
observed.

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
   - Desktop environment + display server (`echo $XDG_SESSION_TYPE`)
   - Output of `journalctl --user -u ag.service -n 50` if the install
     completed but ag.service didn't start
   - Screenshot or terminal output of where the install broke
