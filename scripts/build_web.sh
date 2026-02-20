#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PROFILE="${WEB_PROFILE:-debug}"
if [[ "${1:-}" == "--release" ]]; then
  PROFILE="release"
fi

if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "error: wasm-bindgen not found. Install with: cargo install wasm-bindgen-cli" >&2
  exit 1
fi

if ! rustup target list --installed | grep -q '^wasm32-unknown-unknown$'; then
  echo "error: rust target wasm32-unknown-unknown is not installed." >&2
  echo "install with: rustup target add wasm32-unknown-unknown" >&2
  exit 1
fi

BUILD_FLAGS=()
if [[ "$PROFILE" == "release" ]]; then
  BUILD_FLAGS+=(--release)
fi

echo "Building racing wasm (${PROFILE})..."
cargo build -p racing --bin racing --target wasm32-unknown-unknown "${BUILD_FLAGS[@]}"

WASM_PATH="target/wasm32-unknown-unknown/${PROFILE}/racing.wasm"
OUT_DIR="web-dist"
mkdir -p "$OUT_DIR"

echo "Generating wasm-bindgen output..."
wasm-bindgen \
  --target web \
  --out-dir "$OUT_DIR" \
  "$WASM_PATH"

echo "Copying game assets..."
rm -rf "$OUT_DIR/assets"
cp -R racing/assets "$OUT_DIR/assets"

if [[ ! -f "$OUT_DIR/index.html" ]]; then
  cat > "$OUT_DIR/index.html" <<'HTML'
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Programming Game</title>
    <style>
      html,
      body {
        margin: 0;
        height: 100%;
        background: #111;
      }
      canvas {
        display: block;
      }
    </style>
  </head>
  <body>
    <script type="module">
      import init from './racing.js';
      await init();
    </script>
  </body>
</html>
HTML
fi

echo "Web build ready in ${OUT_DIR}/"
