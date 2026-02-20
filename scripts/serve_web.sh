#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [[ "${NO_BUILD:-0}" != "1" ]]; then
  "${ROOT_DIR}/scripts/build_web.sh" "${1:-}"
fi

: "${RACEHUB_BIND:=127.0.0.1:8787}"
: "${RACEHUB_AUTH_MODE:=required}"
: "${RACEHUB_STATIC_DIR:=web-dist}"

echo "Serving web app from ${RACEHUB_STATIC_DIR} at http://${RACEHUB_BIND}"
RACEHUB_BIND="$RACEHUB_BIND" \
RACEHUB_AUTH_MODE="$RACEHUB_AUTH_MODE" \
RACEHUB_STATIC_DIR="$RACEHUB_STATIC_DIR" \
  cargo run -p racehub
