# ==========
# CONFIG
# ==========

# Backend
BACKEND_DIR := "backend/ag"
BACKEND_CRATE := "ag"

# Frontend
FRONTEND_DIR := "frontend/fro"
FRONTEND_CRATE := "fro"
FRONTEND_DIST := "frontend/fro/dist"

# Tools
WASM_OPT := "wasm-opt"
TRUNK := "trunk"

# ==========
# DEFAULT
# ==========

default: build-all

# ==========
# BACKEND
# ==========

build-backend-release:
    cd {{BACKEND_DIR}} && cargo build --release

build-backend-multi:
    cd {{BACKEND_DIR}} && \
        cargo build --release --target x86_64-unknown-linux-gnu && \
        cargo build --release --target x86_64-unknown-linux-musl

run-backend:
    cd {{BACKEND_DIR}} && cargo run --release

# ==========
# FRONTEND (DEV)
# ==========

dev-frontend:
    cd {{FRONTEND_DIR}} && {{TRUNK}} serve

# ==========
# FRONTEND (RELEASE + WASM-OPT)
# ==========

build-frontend-release:
    cd {{FRONTEND_DIR}} && {{TRUNK}} build --release

optimize-frontend-wasm: build-frontend-release
    # Vind de gegenereerde wasm (Dioxus/Trunk plaatst die in dist/)
    cd {{FRONTEND_DIR}} && \
        WASM_FILE=$$(ls dist/*.wasm | head -n 1) && \
        echo "Optimizing $$WASM_FILE" && \
        {{WASM_OPT}} -O3 --strip-debug --vacuum --merge-blocks --dce --inlining --flatten --rereloop \
            -o dist/app.opt.wasm $$WASM_FILE && \
        mv dist/app.opt.wasm $$WASM_FILE

# ==========
# RELEASE BUNDLE
# ==========

release-bundle: build-backend-release optimize-frontend-wasm
    rm -rf release
    mkdir -p release/backend
    mkdir -p release/frontend

    # Backend binary
    cp {{BACKEND_DIR}}/target/release/{{BACKEND_CRATE}} release/backend/

    # Frontend dist (incl. geoptimaliseerde wasm)
    cp -r {{FRONTEND_DIST}}/* release/frontend/

    # Compressie (gzip + brotli) voor frontend assets
    cd release/frontend && \
        find . -type f \( -name "*.wasm" -o -name "*.js" -o -name "*.css" -o -name "*.html" \) -print0 | \
        xargs -0 -I{} sh -c 'gzip -kf "{}"; brotli -kf "{}"'

# ==========
# FULL PIPELINE
# ==========

build-all: build-backend-release optimize-frontend-wasm

release-all: release-bundle

clean:
    cd {{BACKEND_DIR}} && cargo clean
    cd {{FRONTEND_DIR}} && cargo clean
    rm -rf release

