#!/usr/bin/env bash
set -euo pipefail

PLUGIN_ID="custom-gears"
CRATE_LIB="solidcraft_gear_plugin"
DEST_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/solidcraft/plugins/${PLUGIN_ID}"

echo "[1/4] Building wasm32-unknown-unknown..."
cargo build --target wasm32-unknown-unknown --release

echo "[2/4] Embedding WIT & creating WebAssembly Component..."
mkdir -p "dist/${PLUGIN_ID}"
wasm-tools component embed wit/ \
  "target/wasm32-unknown-unknown/release/${CRATE_LIB}.wasm" \
  -o target/embedded.wasm
wasm-tools component new target/embedded.wasm \
  -o "dist/${PLUGIN_ID}/plugin.wasm"
cp plugin.json "dist/${PLUGIN_ID}/"

echo "[3/4] Packaging zip..."
(cd dist && rm -f "${PLUGIN_ID}.zip" && zip -rq "${PLUGIN_ID}.zip" "${PLUGIN_ID}/")

echo "[4/4] Installing to ${DEST_DIR}..."
mkdir -p "${DEST_DIR}"
cp -f "dist/${PLUGIN_ID}/plugin.json" "dist/${PLUGIN_ID}/plugin.wasm" "${DEST_DIR}/"

echo "Done! Installed to ${DEST_DIR}"