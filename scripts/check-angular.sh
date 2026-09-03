#!/usr/bin/env bash
# Integration test of the whole chain without a simulator:
# TypeScript -> ngc -> esbuild -> QuickJS -> shadow tree -> taffy.
#
# It checks the shape of the resolved tree, not pixels: that Angular started,
# that signals move the screen, that `@for` and `@if` produce the right native
# nodes, and that a tap makes it back to a signal.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="${1:-examples/hello-angular}"
NAME="$(basename "$APP")"

cd "$ROOT"
cargo an build "$APP" >/dev/null
OUTPUT="$(cargo run -q -p an-bridge --example headless -- "build/bundle/$NAME/main.js" 6 2>&1)"

fail=0
check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    echo "  ok   $2"
  else
    echo "  FAIL $2"
    fail=1
  fi
}

echo "== $NAME"
check 'Angular is running in development mode' 'Angular started'
check 'Text#[0-9]+ .*"angular-native"' 'the title reached a native Text node'
check 'View#[0-9]+ \[16,115 361x88\]' 'the row landed where it should'
check 'View#[0-9]+ \[0,0 116x88\]' '@for: first card with flexGrow 1'
check 'View#[0-9]+ \[128,0 233x88\]' '@for: second card with flexGrow 2'
check 'seconds running: 5' 'the signal advanced five frames'
check 'The @if came in at 3 seconds' '@if mounted its branch once past the threshold'
check 'taps: 1 \(last at 40, 20\)' 'a native tap made it all the way to the signal'
check '\-\- frame 4 \(t=4000ms\): 1 operation' 'a settled frame costs a single operation'

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT"
  exit 1
fi
