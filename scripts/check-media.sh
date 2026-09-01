#!/usr/bin/env bash
# Mapa y vídeo: que se monten como su clase de nodo y se repartan el hueco.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
cargo an build examples/media >/dev/null
OUTPUT="$(cargo run -q -p an-bridge --example headless -- build/bundle/media/main.js 3 2>&1)"

fail=0
check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    echo "  ok   $2"
  else
    echo "  FALLO $2"
    fail=1
  fi
}

check 'MapView#[0-9]+ \[[0-9]+,[0-9]+ 361x[0-9]+\]' 'el mapa se monta y se reparte el hueco'
check 'VideoView#[0-9]+ \[[0-9]+,[0-9]+ 361x200\]' 'el vídeo se monta con su alto'

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT"
  exit 1
fi
