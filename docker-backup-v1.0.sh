#!/bin/bash
# =============================================================================
# docker-backup-v1.0.sh
# Location: ~/ag/installer/docker-backup-v1.0.sh
# Purpose: Full Docker Desktop backup before migrating to Docker Engine
# Usage:   chmod +x docker-backup-v1.0.sh && ./docker-backup-v1.0.sh
# Version: 1.0
# =============================================================================

set -euo pipefail

# ── Config ────────────────────────────────────────────────────────────────────
BACKUP_ROOT="$HOME/docker-backup"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="$BACKUP_ROOT/$TIMESTAMP"
LOG="$BACKUP_DIR/backup.log"
GRAFANA_URL="http://localhost:3001"
GRAFANA_AUTH="admin:admin"           # change if you changed the default
AG_DIR="$HOME/ag"

# ── Colours ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

log()  { echo -e "${GREEN}[✓]${NC} $1" | tee -a "$LOG"; }
warn() { echo -e "${YELLOW}[!]${NC} $1" | tee -a "$LOG"; }
err()  { echo -e "${RED}[✗]${NC} $1" | tee -a "$LOG"; }
info() { echo -e "${CYAN}[→]${NC} $1" | tee -a "$LOG"; }

# ── Setup ─────────────────────────────────────────────────────────────────────
mkdir -p "$BACKUP_DIR"/{images,volumes,configs,grafana-dashboards}
touch "$LOG"

echo ""
echo -e "${CYAN}================================================${NC}"
echo -e "${CYAN}  Docker Desktop → Engine  |  Backup Script    ${NC}"
echo -e "${CYAN}  Version 1.0  |  $(date)   ${NC}"
echo -e "${CYAN}================================================${NC}"
echo ""
info "Backup directory: $BACKUP_DIR"
echo ""

# ── Step 1: Snapshot running state ───────────────────────────────────────────
echo -e "${CYAN}── Step 1: Snapshot current Docker state ──${NC}"

docker ps -a        > "$BACKUP_DIR/docker-ps-all.txt"      && log "Saved: docker ps -a"
docker images       > "$BACKUP_DIR/docker-images.txt"      && log "Saved: docker images"
docker volume ls    > "$BACKUP_DIR/docker-volumes.txt"     && log "Saved: docker volume ls"
docker network ls   > "$BACKUP_DIR/docker-networks.txt"    && log "Saved: docker network ls"
docker info         > "$BACKUP_DIR/docker-info.txt"        && log "Saved: docker info"

echo ""

# ── Step 2: Save Docker images ────────────────────────────────────────────────
echo -e "${CYAN}── Step 2: Save Docker images ──${NC}"

# Get all image repo:tag pairs (skip <none>)
IMAGES=$(docker images --format "{{.Repository}}:{{.Tag}}" | grep -v '<none>' || true)

if [ -z "$IMAGES" ]; then
    warn "No images found to save."
else
    while IFS= read -r image; do
        # Make a safe filename
        safe_name=$(echo "$image" | tr '/:' '__')
        out="$BACKUP_DIR/images/${safe_name}.tar"
        info "Saving image: $image → $out"
        if docker save -o "$out" "$image" 2>>"$LOG"; then
            log "Saved: $image ($(du -sh "$out" | cut -f1))"
        else
            err "Failed to save: $image"
        fi
    done <<< "$IMAGES"
fi

echo ""

# ── Step 3: Backup named volumes ──────────────────────────────────────────────
echo -e "${CYAN}── Step 3: Backup named volumes ──${NC}"

VOLUMES=$(docker volume ls --format "{{.Name}}" || true)

if [ -z "$VOLUMES" ]; then
    warn "No named volumes found."
else
    while IFS= read -r vol; do
        out="$BACKUP_DIR/volumes/${vol}.tar.gz"
        info "Backing up volume: $vol"
        if docker run --rm \
            -v "${vol}:/source:ro" \
            -v "$BACKUP_DIR/volumes:/backup" \
            alpine tar czf "/backup/${vol}.tar.gz" -C /source . 2>>"$LOG"; then
            log "Saved volume: $vol ($(du -sh "$out" | cut -f1))"
        else
            err "Failed to backup volume: $vol (may be empty or inaccessible)"
        fi
    done <<< "$VOLUMES"
fi

echo ""

# ── Step 4: Save compose files and project configs ────────────────────────────
echo -e "${CYAN}── Step 4: Save project config files ──${NC}"

FILES_TO_COPY=(
    "$AG_DIR/docker-compose.yml"
    "$AG_DIR/docker-compose.observability.yml"
    "$AG_DIR/otel-collector-config.yaml"
    "$AG_DIR/.env"
    "$AG_DIR/.env.example"
)

for f in "${FILES_TO_COPY[@]}"; do
    if [ -f "$f" ]; then
        cp "$f" "$BACKUP_DIR/configs/"
        log "Saved: $f"
    else
        warn "Not found (skipping): $f"
    fi
done

# Monitoring directory (Grafana provisioning, Prometheus rules etc.)
if [ -d "$AG_DIR/monitoring" ]; then
    cp -r "$AG_DIR/monitoring" "$BACKUP_DIR/configs/monitoring"
    log "Saved: $AG_DIR/monitoring/"
else
    warn "No monitoring/ directory found."
fi

# Docker daemon config
for daemon_cfg in "$HOME/.docker/daemon.json" "/etc/docker/daemon.json"; do
    if [ -f "$daemon_cfg" ]; then
        cp "$daemon_cfg" "$BACKUP_DIR/configs/daemon.json"
        log "Saved daemon config: $daemon_cfg"
    fi
done

echo ""

# ── Step 5: Export Grafana dashboards via API ─────────────────────────────────
echo -e "${CYAN}── Step 5: Export Grafana dashboards ──${NC}"

if curl -sf "$GRAFANA_URL/api/health" > /dev/null 2>&1; then
    # Get all dashboard UIDs
    UIDS=$(curl -s "$GRAFANA_URL/api/search?type=dash-db" \
        -u "$GRAFANA_AUTH" \
        | grep -o '"uid":"[^"]*"' | cut -d'"' -f4 || true)

    if [ -z "$UIDS" ]; then
        warn "No Grafana dashboards found (or jq not available)."
    else
        while IFS= read -r uid; do
            out="$BACKUP_DIR/grafana-dashboards/${uid}.json"
            if curl -sf "$GRAFANA_URL/api/dashboards/uid/$uid" \
                -u "$GRAFANA_AUTH" -o "$out" 2>>"$LOG"; then
                log "Saved dashboard: $uid"
            else
                err "Failed to export dashboard: $uid"
            fi
        done <<< "$UIDS"
    fi
else
    warn "Grafana not reachable at $GRAFANA_URL — skipping dashboard export."
    warn "If Grafana is running on a different port, edit GRAFANA_URL at top of script."
fi

echo ""

# ── Step 6: Summary ───────────────────────────────────────────────────────────
echo -e "${CYAN}── Backup Summary ──${NC}"
echo ""

TOTAL=$(du -sh "$BACKUP_DIR" | cut -f1)

echo -e "${GREEN}================================================${NC}"
echo -e "${GREEN}  Backup Complete!${NC}"
echo -e "${GREEN}================================================${NC}"
echo ""
echo -e "  Location : ${CYAN}$BACKUP_DIR${NC}"
echo -e "  Total    : ${CYAN}$TOTAL${NC}"
echo -e "  Log      : ${CYAN}$LOG${NC}"
echo ""
echo "  Contents:"
ls -lh "$BACKUP_DIR"
echo ""
echo -e "${YELLOW}  You can now safely proceed with:${NC}"
echo "    sudo apt-get remove docker-desktop -y"
echo ""

# ── Restore instructions ──────────────────────────────────────────────────────
cat > "$BACKUP_DIR/RESTORE.md" << 'EOF'
# Docker Backup Restore Instructions

## Restore Images
```bash
for f in images/*.tar; do docker load -i "$f"; done
```

## Restore a Volume
```bash
docker volume create <volume_name>
docker run --rm \
  -v <volume_name>:/target \
  -v $(pwd)/volumes:/backup \
  alpine tar xzf /backup/<volume_name>.tar.gz -C /target
```

## Restore All Volumes
```bash
for f in volumes/*.tar.gz; do
  vol=$(basename "$f" .tar.gz)
  docker volume create "$vol"
  docker run --rm \
    -v "${vol}:/target" \
    -v "$(pwd)/volumes:/backup" \
    alpine tar xzf "/backup/${vol}.tar.gz" -C /target
  echo "Restored: $vol"
done
```

## Restart Stack
```bash
cd ~/ag
docker compose -f docker-compose.observability.yml up -d
docker compose up -d
```

## Restore Grafana Dashboards
```bash
for f in grafana-dashboards/*.json; do
  curl -s -X POST http://admin:admin@localhost:3001/api/dashboards/import \
    -H "Content-Type: application/json" \
    -d "{\"dashboard\": $(jq '.dashboard' $f), \"overwrite\": true, \"folderId\": 0}"
done
```
EOF

log "Saved: RESTORE.md"
echo ""
