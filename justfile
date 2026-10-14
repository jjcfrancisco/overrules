#!/usr/bin/env just --justfile
# Recipes for working on overrules. `just` lists them.

set shell := ['bash', '-euo', 'pipefail', '-c']

# The Overture release the Seville extract (and the test fixture) comes from.
release := '2026-09-23.1'
seville := 'data/seville/segment.parquet'

# Minimum supported Rust version, read from Cargo.toml.
msrv := `sed -n 's/^rust-version = "\(.*\)"/\1/p' Cargo.toml`

wasm_target := 'wasm32-unknown-unknown'

# Apple's `ar` silently drops non-Mach-O members, losing psm's precompiled wasm32 object
# (opening-hours → pest → stacker → psm). Builds for wasm32 on macOS need LLVM's.
llvm_ar := if os() == 'macos' { `ls -d /opt/homebrew/opt/llvm*/bin/llvm-ar /usr/local/opt/llvm*/bin/llvm-ar 2>/dev/null | head -n1 || true` } else { '' }
wasm_env := if llvm_ar != '' { 'AR_wasm32_unknown_unknown=' + llvm_ar } else { '' }

@_default:
    just --list --unsorted

# ─── develop ─────────────────────────────────────────────────────────────────

# Run all tests: unit, conformance vectors, doc tests
test *args:
    cargo test --all-features {{ args }}

# Format the code
fmt:
    cargo fmt --all

# Check formatting without changing anything
fmt-check:
    cargo fmt --all -- --check

# Lint with clippy; warnings are errors
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Check each feature set compiles on its own
check-features:
    cargo check --no-default-features
    cargo check --all-features --all-targets

# Check the library still compiles for the browser (wasm32), as the explorer needs
check-wasm: _wasm-target
    {{ wasm_env }} cargo clippy --target {{ wasm_target }} -- -D warnings

# Build the API docs; broken links are errors
doc *args:
    RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --all-features {{ args }}

# Check the minimum supported Rust version still builds
msrv:
    rustup toolchain list | grep -q '^{{ msrv }}' || rustup toolchain install {{ msrv }} --profile minimal
    cargo +{{ msrv }} check --all-targets --all-features

# Check licences, advisories and sources (needs cargo-deny)
deny:
    cargo deny check

# Build the crate exactly as crates.io would receive it, and test that
package:
    cargo package --allow-dirty
    @echo; cargo package --allow-dirty --list | sed 's/^/  /'

# Everything CI runs
ci: fmt-check clippy check-features test doc check-wasm package

# ─── data ────────────────────────────────────────────────────────────────────

# Download the Seville extract from Overture's S3 bucket (needs DuckDB; about 2 minutes)
data:
    mkdir -p data/seville
    duckdb < data/extract_seville.sql

# Rebuild the test fixture from the Seville extract
fixtures:
    cargo run --release -q --example export --features parquet -- {{ seville }} \
        99698abc e26d3384 1b946971 645f3ec7 a022afe5 55c99d8b \
        > tests/fixtures/seville.geojson

# Resolve on the Seville extract, e.g. `just resolve e26d3384 --at 0.05 --heading forward --mode car`
resolve *args:
    cargo run --release -q --example resolve --features parquet -- {{ seville }} {{ args }}

# How far planar maths would move every `between` boundary in Seville
audit:
    cargo run --release -q --example planar_audit --features parquet -- {{ seville }}

# Remove build outputs
clean:
    cargo clean

_wasm-target:
    rustup target list --installed | grep -q '^{{ wasm_target }}$' || rustup target add {{ wasm_target }}
    {{ if os() == 'macos' { if llvm_ar == '' { 'echo "wasm32 builds on macOS need llvm-ar: brew install llvm" >&2; exit 1' } else { 'true' } } else { 'true' } }}
