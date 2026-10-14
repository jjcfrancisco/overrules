#!/usr/bin/env just --justfile
# Recipes for working on overrules. `just` lists them.

set shell := ['bash', '-euo', 'pipefail', '-c']

# The Overture release the demo data comes from, and the segments the CLI reads by default.
release := '2026-09-23.1'
export OVERRULES_SEGMENTS := env('OVERRULES_SEGMENTS', 'data/seville/segment.parquet')

# Must match the `wasm-bindgen` version pinned in Cargo.toml.
wasm_bindgen_version := '0.2.123'
wasm_target := 'wasm32-unknown-unknown'
dist := 'dist'

# Minimum supported Rust version, read from Cargo.toml.
msrv := `sed -n 's/^rust-version = "\(.*\)"/\1/p' Cargo.toml`

# Apple's `ar` silently drops non-Mach-O members, losing psm's precompiled wasm32 object
# (opening-hours → pest → stacker → psm). Builds for wasm32 on macOS need LLVM's.
llvm_ar := if os() == 'macos' { `ls -d /opt/homebrew/opt/llvm*/bin/llvm-ar /usr/local/opt/llvm*/bin/llvm-ar 2>/dev/null | head -n1 || true` } else { '' }
wasm_env := if llvm_ar != '' { 'AR_wasm32_unknown_unknown=' + llvm_ar } else { '' }

@_default:
    just --list --unsorted

# ─── develop ─────────────────────────────────────────────────────────────────

# Build every native crate
build:
    cargo build --workspace

# Run all tests (unit, integration, conformance vectors, doc tests)
test *args:
    cargo test --workspace --all-features {{ args }}

# Format the code
fmt:
    cargo fmt --all

# Check formatting without changing anything
fmt-check:
    cargo fmt --all -- --check

# Lint with clippy; warnings are errors
clippy:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Lint the WebAssembly crate for its real target
clippy-wasm: _wasm-target
    {{ wasm_env }} cargo clippy -p overrules-wasm --target {{ wasm_target }} -- -D warnings

# Check every feature combination of the library compiles on its own
check-features:
    cargo check -p overrules --no-default-features
    cargo check -p overrules --features parquet
    cargo check -p overrules --features pack
    cargo check -p overrules-explorer --no-default-features
    cargo check -p overrules-cli --no-default-features

# Build the API docs; broken links are errors
doc *args:
    RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --all-features {{ args }}

# Check the minimum supported Rust version still builds
msrv:
    rustup toolchain list | grep -q '^{{ msrv }}' || rustup toolchain install {{ msrv }} --profile minimal
    cargo +{{ msrv }} check --workspace --all-targets --all-features

# Check licences, advisories and duplicate dependencies (needs cargo-deny)
deny:
    cargo deny check

# Everything CI runs, in the same order
ci: fmt-check clippy check-features test doc clippy-wasm wasm-smoke

# ─── data ────────────────────────────────────────────────────────────────────

# Download the Seville extract from Overture's S3 bucket (needs DuckDB; about 2 minutes)
data:
    mkdir -p data/seville
    duckdb < data/extract_seville.sql

# Rebuild the test fixture from the Seville extract
fixtures:
    cargo run --release -q -p overrules-cli -- export \
        99698abc e26d3384 1b946971 645f3ec7 a022afe5 55c99d8b \
        > crates/core/tests/fixtures/seville.geojson

# ─── run ─────────────────────────────────────────────────────────────────────

# Run the CLI, e.g. `just run resolve --segment e26d3384 --at 0.05 --heading forward --mode car`
run *args:
    cargo run --release -q -p overrules-cli -- {{ args }}

# Serve the explorer at http://127.0.0.1:<port>
serve port='8787':
    cargo run --release -q -p overrules-cli -- serve --port {{ port }}

# ─── web ─────────────────────────────────────────────────────────────────────

# Build the static explorer into dist/: the page, the resolver as WebAssembly, and a data pack
web: wasm
    cp crates/explorer/web/* {{ dist }}/
    cargo run --release -q -p overrules-cli -- pack --out {{ dist }}/seville.pack.gz --source "Seville · Overture {{ release }}"
    @echo; du -h {{ dist }}/* {{ dist }}/pkg/* | sort -k2
    @echo; echo "try:  just web-serve   →  http://127.0.0.1:8788/"

# Compile the resolver to WebAssembly and generate its JS bindings into dist/pkg
wasm: _wasm-target _wasm-bindgen
    {{ wasm_env }} cargo build -p overrules-wasm --profile wasm-release --target {{ wasm_target }}
    rm -rf {{ dist }}/pkg && mkdir -p {{ dist }}
    wasm-bindgen --target web --no-typescript --out-dir {{ dist }}/pkg \
        target/{{ wasm_target }}/wasm-release/overrules_wasm.wasm
    if command -v wasm-opt >/dev/null; then wasm-opt -O3 -o {{ dist }}/pkg/overrules_wasm_bg.wasm {{ dist }}/pkg/overrules_wasm_bg.wasm; fi

# Serve dist/ as static files, the way GitHub Pages would
web-serve port='8788':
    python3 -m http.server -d {{ dist }} {{ port }}

# Load the WebAssembly build in Node and check its answers against the test fixture
wasm-smoke: wasm
    cargo run --release -q -p overrules-cli -- --segments crates/core/tests/fixtures/seville.geojson \
        pack --out target/fixture.pack.gz --source fixture
    node crates/wasm/tests/smoke.mjs {{ dist }} target/fixture.pack.gz

# Remove build outputs
clean:
    cargo clean
    rm -rf {{ dist }}

_wasm-target:
    rustup target list --installed | grep -q '^{{ wasm_target }}$' || rustup target add {{ wasm_target }}
    {{ if os() == 'macos' { if llvm_ar == '' { 'echo "wasm32 builds on macOS need llvm-ar: brew install llvm" >&2; exit 1' } else { 'true' } } else { 'true' } }}

_wasm-bindgen:
    #!/usr/bin/env bash
    set -euo pipefail
    have=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)
    if [[ "$have" != "{{ wasm_bindgen_version }}" ]]; then
        echo "installing wasm-bindgen-cli {{ wasm_bindgen_version }} (found: ${have:-none})"
        cargo install wasm-bindgen-cli --version {{ wasm_bindgen_version }} --locked
    fi
