#!/usr/bin/env bash
# That the border a template asks for is the border it gets, or is told it is not.
#
# There are two borders with almost the same name and they do opposite things.
# `[borderWidth]` is a prop: it reaches every host, which strokes one line
# inside the frame, and it moves no child. `borderTopWidth` and its three
# siblings are *styles*: taffy resolves them, they push the children in, and
# they stop in the core — no host is told anything, so no line is drawn.
#
# That second half used to happen in silence, which is the expensive kind of
# gap: the children move, so something clearly happened, and the line that was
# asked for is simply absent. The core now says it once per name, and this
# checks the three claims together — the inset, the absence, and the sentence.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="${1:-examples/borders}"
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

echo "== the border that is drawn"
# The prop arrives, with its colour, and the child stays where it was: a
# `CALayer` border is painted over the content and does not reserve room.
check 'props borderColor=#6ee7b7 borderWidth=4 testID=drawn' 'the [borderWidth] prop reaches the host'
check 'View#[0-9]+ \[0,0 200x60\]  props backgroundColor=#1e293b testID=drawn-child' \
  'the drawn border moves no child'

echo
echo "== the four that are layout"
# 2 / 4 / 8 / 16 clockwise from the top: the child starts 16 from the left and 2
# from the top, and loses 20 of width and 10 of height.
check 'View#[0-9]+ \[16,2 180x50\]  props backgroundColor=#1e293b testID=inset-child' \
  'the four per-side widths inset the children by exactly what they say'

# The other half, and the one a check has to make: nothing with that name left
# the core. If a host ever started reading it, the dump would show it here.
if grep -qE 'border(Top|Right|Bottom|Left)Width' <<<"$(grep -E 'testID=inset\b' <<<"$OUTPUT" || true)"; then
  echo "  FAIL a per-side border width reached a host as a prop"
  fail=1
else
  echo "  ok   no per-side border width reaches a host, so there is nothing to draw with"
fi

echo
echo "== and it is said out loud"
# Once per name, not once per node and not once per frame: the styles are
# written again on every change detection pass.
for side in borderTopWidth borderRightWidth borderBottomWidth borderLeftWidth; do
  said="$(grep -cF "[style.$side] insets this node's children and draws nothing" <<<"$OUTPUT" || true)"
  if [ "$said" = "1" ]; then
    echo "  ok   $side says once that it insets and draws nothing"
  else
    echo "  FAIL $side said it $said times; it must be said exactly once"
    fail=1
  fi
done

echo
echo "== nobody half-draws one"
# The refusal is the same on all four hosts, and that is the point of it: a host
# that quietly grew a per-side border would make the sentence above a lie on
# three platforms out of four and true on the fourth, which is worse than none.
for host in crates/an-ios/src crates/an-macos/src crates/an-watch/src \
            shells/android/java shells/watchos/Sources; do
  hits="$(grep -rlE '"border(Top|Right|Bottom|Left)Width"' "$host" 2>/dev/null || true)"
  if [ -n "$hits" ]; then
    echo "  FAIL $host reads a per-side border width:"
    echo "$hits" | sed 's/^/    /'
    fail=1
  else
    echo "  ok   $host draws no per-side border"
  fi
done

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT" | head -30
  exit 1
fi
