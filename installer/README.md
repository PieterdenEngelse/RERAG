# ag-installer

GUI installer for ag, distributed as a Linux AppImage. Dioxus desktop UI;
ships pre-built ag binaries inside the AppImage so end-users don't need
rustc/cargo/npm/trunk.

The terminal installer at `installers/install-linux.sh` is the parallel
developer path. Same XDG layout, same systemd units, same detection
rules — different driver.

## Where to look

- **Design + execution plan**: `docs/bin3` (in the repo root).
- **Phase 0 lock-ins**: `docs/bin3` §Phase 0 (six decisions: FalkorDB
  image SHA, Ubuntu runner, semver start, release publish mode,
  `.desktop` categories, auto-update channel).
- **Build pipeline**: `.github/workflows/release.yml` — tag-driven
  CI builds the AppImage on `ubuntu-22.04`.
- **AppImage bundle script**: `installer/build-appimage.sh` — assembles
  `AppDir/`, runs `appimagetool`. Called by CI; runnable locally.
- **FalkorDB image pin**: `installer/falkordb.pin` — pinned
  `image@sha256:…` for the FalkorDB binaries extracted at build time.

## Phase A status (this commit)

Foundation only: a Dioxus desktop window that prints version + git SHA
+ build timestamp. The six screens (Welcome → Detection → Prompts →
Install Progress → First-Run Config → Done) land in Phase B onward.

### Local development

```bash
cargo run -p ag-installer
# or
cargo build --release -p ag-installer
./target/release/ag-installer --version
./target/release/ag-installer            # opens the window
```

### Building an AppImage locally

```bash
# Prereqs: cargo, appimagetool on PATH, docker (for FalkorDB extract)
cargo build --release -p ag
cargo build --release -p ag-installer
(cd frontend/fro && npm ci && npm run css:build && trunk build --release)

# Extract FalkorDB binaries from the pinned image
PIN="$(grep -v '^#' installer/falkordb.pin | head -1 | tr -d '[:space:]')"
docker pull "$PIN"
mkdir -p installer/stage/falkordb
docker create --name ag-fdb-extract "$PIN"
docker cp ag-fdb-extract:/usr/local/bin/redis-server          installer/stage/falkordb/
docker cp ag-fdb-extract:/usr/local/bin/redis-cli             installer/stage/falkordb/
docker cp ag-fdb-extract:/var/lib/falkordb/bin/falkordb.so    installer/stage/falkordb/
docker rm ag-fdb-extract

# Assemble + bundle
bash installer/build-appimage.sh
# → produces ag-installer-v<version>-x86_64.AppImage in the repo root
```

### Running the AppImage

```bash
chmod +x ag-installer-v*.AppImage
./ag-installer-v*.AppImage --version
./ag-installer-v*.AppImage           # opens the window
```

## Versioning

Semver: `vMAJOR.MINOR.PATCH`. Starting at `v0.1.0`. Any tag matching
`v0.*.*` is published as a GitHub pre-release (yellow badge) so the
auto-update channel can filter stable vs. pre-release correctly.
`v1.0.0` ships only when Phase H acceptance is met.
