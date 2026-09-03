#!/usr/bin/env bash
# The list app: text field, ScrollView and a windowed list.
#
# What it really verifies is that five thousand rows are not five thousand views.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
cargo an build examples/kitchen >/dev/null
OUTPUT="$(cargo run -q -p an-bridge --example headless -- build/bundle/kitchen/main.js 6 2>&1)"

fail=0
check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    echo "  ok   $2"
  else
    echo "  FAIL $2"
    fail=1
  fi
}

echo "== kitchen"
check 'TextInput#[0-9]+ \[16,70 361x40\]' 'the text field was measured and placed'
# The configurable field. Which keyboard comes up is not decoration: an email
# field with the plain text keyboard forces you to hunt for the at sign, and on
# Android the four props are flags of the same integer, so either they all
# arrive or none does.
check 'TextInput#[0-9]+ .*autoCapitalize=none autoCorrect=false' 'the field asks for a keyboard with no caps and no autocorrect'
check 'TextInput#[0-9]+ .*keyboardType=default .*returnKeyType=search' 'and the return key says "search"'
check 'TextInput#[0-9]+ .*placeholderColor=#6b7a99' 'the hint text carries a colour of its own'
check 'TextInput#[0-9]+ .*ios:clearButtonMode=whileEditing' 'the clear cross travels marked as an iOS one'
check 'TextInput#[0-9]+ .*android:selectAllOnFocus=true' 'and select-on-focus as an Android one'
check '"headless 0.0 . en-GB"' 'the native module answered and the promise resolved'
check 'ScrollView#[0-9]+ \[0,0 393x666\]' 'the ScrollView fills the gap, it does not grow with its content'
check 'content 393x312000' 'the contentSize adds up row by row: 4000 of 56 and 1000 of 88'
check '"row number 6[0-9]"' 'after scrolling, the rows at that height are the ones on screen'
check 'View#[0-9]+ \[0,3744 393x88\]' 'the tall row measures 88'
check 'View#[0-9]+ \[0,3832 393x56\]' 'the next one starts right below the tall one'
check 'scrolling cost 0 views created and 0 destroyed' 'scrolling recycles: not one new view'
if grep -qE -- '"row number (1|2|300)"' <<<"$OUTPUT"; then
  echo "  FAIL after scrolling nothing from the start or the end should be left"
  fail=1
else
  echo "  ok   nothing outside the window is mounted"
fi
mounted="$(grep -oE 'native views mounted: [0-9]+' <<<"$OUTPUT" | grep -oE '[0-9]+')"
if [ -n "$mounted" ] && [ "$mounted" -lt 120 ]; then
  echo "  ok   $mounted native views for 5000 rows"
else
  echo "  FAIL too many views mounted: ${mounted:-?}"
  fail=1
fi

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT"
  exit 1
fi
