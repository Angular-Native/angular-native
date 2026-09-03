#!/usr/bin/env bash
# Map and video: that they mount as their own node kind and take their share.
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
    echo "  FAIL $2"
    fail=1
  fi
}

check 'MapView#[0-9]+ \[[0-9]+,[0-9]+ 361x[0-9]+\]' 'the map mounts and takes its share of the gap'
check 'VideoView#[0-9]+ \[[0-9]+,[0-9]+ 361x200\]' 'the video mounts with its own height'

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT"
  exit 1
fi
