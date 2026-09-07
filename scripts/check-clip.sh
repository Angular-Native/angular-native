#!/usr/bin/env bash
# That the app's body is a body and not a canvas.
#
# `overflow` used to be resolved in taffy and stop there: no host was ever told
# that a node clips, so children were painted outside their parents. And a
# `ScrollView` whose content came out a few points wider than its frame was
# offering horizontal scrolling nobody asked for, which is how a page can be
# dragged sideways until the screen is empty.
#
# Both are checked over the headless dump rather than on a device, because that
# is where the core's decision is visible: `clip` is the op the host was sent,
# and `content` is the size it was told to use.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="${1:-examples/feed}"
NAME="$(basename "$APP")"
cd "$ROOT"

cargo an build "$APP" >/dev/null
OUTPUT="$(cargo run -q -p an-bridge --example headless -- "build/bundle/$NAME/main.js" 3 2>&1)"

fail=0
check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    echo "  ok   $2"
  else
    echo "  FAIL $2"
    fail=1
  fi
}

echo "== clipping"
# The root carries the marker whatever the template asked for: nothing an app
# does may paint outside the window.
check '^View#[0-9]+ \[0,0 [0-9]+x[0-9]+\]  clip' 'the root clips, because the root is the body'
check 'ScrollView#[0-9]+ .*\]  clip' 'a ScrollView clips, so its content cannot be dragged out of it'

# The other half of the claim: the flag is read, not stamped on everything. If
# every node came back clipped, the two lines above would mean nothing.
if grep -qE '^ +View#[0-9]+ \[[0-9]+,[0-9]+ [0-9]+x[0-9]+\](  props|$)' <<<"$OUTPUT"; then
  echo "  ok   an ordinary container is not clipped just in case"
else
  echo "  FAIL every node came back clipped; the flag is not being read"
  fail=1
fi

# A page that overflows downwards must not also offer to scroll sideways.
sideways="$(awk '
  match($0, /ScrollView#[0-9]+ \[[-0-9.]+,[-0-9.]+ [0-9.]+x/) {
    line = $0
    sub(/.*ScrollView#/, "", line); id = line + 0
    w = line; sub(/^[0-9]+ \[[-0-9.]+,[-0-9.]+ /, "", w); sub(/x.*/, "", w)
    if (match($0, /content [0-9.]+x/)) {
      c = substr($0, RSTART + 8); sub(/x.*/, "", c)
      if (c + 0 > w + 0.5) printf "    ScrollView#%d: frame %s wide, content %s wide\n", id, w, c
    }
  }' <<<"$OUTPUT")"
if [ -n "$sideways" ]; then
  echo "  FAIL a ScrollView is offering horizontal scrolling:"
  echo "$sideways"
  fail=1
else
  echo "  ok   no ScrollView is wider inside than out, so nothing scrolls sideways"
fi

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT" | head -30
  exit 1
fi
