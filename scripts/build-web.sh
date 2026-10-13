#!/usr/bin/env bash
# Build the static web demo into dist/: the page, the resolver compiled to
# WebAssembly, and the Seville data pack. Serve dist/ from any static host.
set -euo pipefail
cd "$(dirname "$0")/.."

OUT=dist
TARGET=wasm32-unknown-unknown
SEGMENTS=${SEGMENTS:-data/seville/segment.parquet}

# Apple's `ar` silently drops non-Mach-O members, which loses psm's precompiled
# wasm32.o (pulled in via opening-hours → pest → stacker). Use llvm-ar instead.
if [[ "$(uname)" == "Darwin" && -z "${AR_wasm32_unknown_unknown:-}" ]]; then
  for c in "$(brew --prefix llvm 2>/dev/null || true)/bin/llvm-ar" /opt/homebrew/opt/llvm*/bin/llvm-ar; do
    if [[ -x "$c" ]]; then export AR_wasm32_unknown_unknown="$c"; break; fi
  done
  [[ -n "${AR_wasm32_unknown_unknown:-}" ]] || { echo "wasm32 build needs llvm-ar: brew install llvm" >&2; exit 1; }
fi

rustup target list --installed | grep -q "^$TARGET$" || rustup target add "$TARGET"
command -v wasm-bindgen >/dev/null || { echo "need wasm-bindgen-cli 0.2.123: cargo install wasm-bindgen-cli --version 0.2.123" >&2; exit 1; }

cargo build --release                                             # native CLI, for `pack`
cargo build --profile wasm-release --lib --target "$TARGET" --no-default-features --features wasm

rm -rf "$OUT/pkg"; mkdir -p "$OUT"
wasm-bindgen --target web --no-typescript --out-dir "$OUT/pkg" "target/$TARGET/wasm-release/gers_lr.wasm"
if command -v wasm-opt >/dev/null; then
  wasm-opt -O3 -o "$OUT/pkg/gers_lr_bg.wasm" "$OUT/pkg/gers_lr_bg.wasm"
fi
cp web/index.html web/style.css web/app.js web/backend.js "$OUT/"
./target/release/gers-lr --segments "$SEGMENTS" pack --out "$OUT/seville.pack.gz"

echo; echo "dist/ ready:"; du -h "$OUT"/* "$OUT"/pkg/* | sort -k2
echo; echo "try:  python3 -m http.server -d $OUT 8788   →  http://127.0.0.1:8788/"
