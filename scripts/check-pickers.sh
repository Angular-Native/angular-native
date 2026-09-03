#!/usr/bin/env bash
# The controls for choosing: segments, dropdown, stepper, search and date.
#
# What is checked here is that each one mounts as its own node kind and
# measures what it should, not that it looks right: that is only visible on the
# device.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
cargo an build examples/pickers >/dev/null
OUTPUT="$(cargo run -q -p an-bridge --example headless -- build/bundle/pickers/main.js 4 2>&1)"

fail=0
check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    echo "  ok   $2"
  else
    echo "  FAIL $2"
    fail=1
  fi
}

check 'SearchBar#[0-9]+ \[' 'the search bar mounts'
check 'SegmentedControl#[0-9]+ \[' 'the segmented control mounts'
check 'Picker#[0-9]+ \[[0-9]+,[0-9]+ 150x40\]' 'the dropdown mounts with its own size'
check 'Stepper#[0-9]+ \[[0-9]+,[0-9]+ 140x40\]' 'the stepper mounts with its own size'
check 'DatePicker#[0-9]+ \[[0-9]+,[0-9]+ 180x40\]' 'the date picker mounts with its own size'
# The safe area no longer inserts a view in between, so whatever is given to it
# to arrange its children reaches them: 20 from the top, 33 tall, 18 of gap.
check 'SearchBar#[0-9]+ \[20,71' "the safe area's gap separates its children"
check 'last button: text' 'the tap reaches the text-variant button'

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT"
  exit 1
fi
