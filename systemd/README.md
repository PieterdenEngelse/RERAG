# Systemd units for ag

The installer (`installers/install-linux.sh`) renders three composable
user systemd units from the templates in this directory and installs
them under `~/.config/systemd/user/`. Don't `cp` the templates by hand
— the placeholders need substituting.

## Templates

| File | Renders to | Placeholders |
|------|------------|--------------|
| `ag.service.tmpl` | `~/.config/systemd/user/ag.service` | `{{AG_BIN}}`, `{{AG_HOME}}`, `{{AG_ENV}}`, `{{AG_LIB_DIR}}`, `{{BACKEND_PORT}}` |
| `ag-stack.service.tmpl` | `~/.config/systemd/user/ag-stack.service` | `{{COMPOSE_FILE}}`, `{{COMPOSE_PROFILE}}` |
| `falkordb.service.tmpl` | `~/.config/systemd/user/falkordb.service` | `{{AG_HOME}}`, `{{FDB_PORT}}`, `{{FDB_PASS}}` |

## Drop-ins

Both live at `systemd/ag.service.d/` in the repo and are copied
verbatim to `~/.config/systemd/user/ag.service.d/` by the installer
(no placeholders — they're plain `[Unit]` snippets).

| File | What it adds | Skipped by |
|------|--------------|------------|
| `falkordb.conf` | `Wants=` + `After=falkordb.service` | `--no-falkordb` |
| `stack.conf` | `Wants=` + `After=ag-stack.service` | `--no-stack` |

## Dependency chain

```
ag.service
  └── Wants/After ── ag-stack.service  (docker compose up -d)
  └── Wants/After ── falkordb.service  (native redis-server + falkordb.so)
```

Either drop-in can be omitted independently. Removing `falkordb.conf`
means ag boots without a graph store; removing `stack.conf` means ag
boots without the compose observability stack (Loki, Tempo, OTel,
Grafana, Prometheus). Removing both means a bare ag.service that
expects every external service to already exist (the
`--force-compose` / `--no-stack` / `--no-falkordb` paths in the
installer).

## Installing by hand (testing only)

If you want to render a template without running the installer:

```bash
sed \
    -e "s|{{AG_BIN}}|$HOME/.local/bin/ag|g" \
    -e "s|{{AG_HOME}}|$HOME/.local/share/ag|g" \
    -e "s|{{AG_ENV}}|$HOME/.config/ag/ag.env|g" \
    -e "s|{{AG_LIB_DIR}}|$HOME/.local/lib|g" \
    -e "s|{{BACKEND_PORT}}|3010|g" \
    systemd/ag.service.tmpl \
    > ~/.config/systemd/user/ag.service
systemctl --user daemon-reload
systemctl --user enable --now ag.service
```

The installer does the same thing but also handles the binary copy,
libtika location, FalkorDB extraction, and drop-in install.

## See also

- `docs/bin2` — full installer design (reuse policy, disk footprint,
  Phase 1 / Phase 2 split).
- `docs/falkordb-native-service.md` — rationale for FalkorDB-as-systemd
  (not a container), and the binary-extraction fallbacks if the image
  is musl-based on a glibc host.
