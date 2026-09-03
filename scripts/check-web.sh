#!/usr/bin/env bash
# Navigation bar, multi-line text, embedded browser and action sheet.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
cargo an build examples/web >/dev/null
OUTPUT="$(cargo run -q -p an-bridge --example headless -- build/bundle/web/main.js 4 2>&1)"

fail=0
check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    echo "  ok   $2"
  else
    echo "  FAIL $2"
    fail=1
  fi
}

check 'NavigationBar#[0-9]+ \[0,0 393x44\]' 'the navigation bar sticks to the top with its own height'
check 'TextEditor#[0-9]+ \[[0-9]+,[0-9]+ [0-9]+x110\]' 'the multi-line editor is mounted'
# `flex: 1` is the shorthand almost everybody writes. It did not exist as a
# property before —only as a value of `display`— and it was lost on the way: the
# view was left with the height of its content instead of taking its share of
# what was left over.
check 'WebView#[0-9]+ \[[0-9]+,[0-9]+ [0-9]+x568\]' 'the browser takes its share of the gap with flex'
check 'Alert#[0-9]+ \[0,0 0x0\]' 'the action sheet takes up no room in the layout'
check 'last: back' 'the back button of the navigation bar reports'

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT"
  exit 1
fi
