#!/usr/bin/env bash
# What a scroll view does with a size, and which way it goes.
#
# Two holes, both of the silent kind. A `[style.height]` on an
# `<an-scroll-view>` resolved to nothing: the core pinned `flex-basis: 0` on
# every scroll view so its content could not size it, and a non-`auto` basis
# beats `height` on the main axis, so the view came out zero points tall with
# nobody saying why. And there was no way to ask for sideways scrolling at all:
# the content size was clamped to the frame's width on the way out of layout.
#
# Both are decisions of the core's, which is why they are checked over the
# headless dump: the frame is what the host was told to give the view, and
# `content` is the size it was told to scroll over.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo an build examples/scroll >/dev/null
OUTPUT="$(cargo run -q -p an-bridge --example headless -- build/bundle/scroll/main.js 6 2>&1)"

fail=0
check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    echo "  ok   $2"
  else
    echo "  FAIL $2"
    fail=1
  fi
}

echo "== scrolling"
# 160 points asked for, 160 points given, with 812 points of content inside it.
check 'ScrollView#[0-9]+ \[[0-9]+,[0-9]+ [0-9]+x160\].*testID=tall' \
  'a height on a scroll view is the height it gets'
check 'ScrollView#[0-9]+ .*content [0-9]+x812.*testID=tall' \
  'and its content still overflows it rather than sizing it'

# Sideways: the children run along x without the template having said so, and
# the content comes out wider than the frame, which is the whole point.
check 'View#[0-9]+ \[1408,0 120x96\]' \
  'a horizontal scroll view lays its children out along x'
check 'ScrollView#[0-9]+ .*content 1528x96.*testID=sideways$' \
  'the content is wider than the frame and no taller than it'
check 'ScrollView#[0-9]+ .*horizontal=true.*testID=sideways$' \
  'and the host is told which axis it is, since the content size carries no axis'

# The direction is a default and not an override.
check 'View#[0-9]+ \[0,220 20x20\]' \
  'a template that wrote flexDirection keeps it, horizontal or not'

# The old behaviour, unchanged: a plain scroll view overflows downwards and is
# clamped across. This is the line that would have caught the fix if it had
# swapped one hole for another.
check 'ScrollView#[0-9]+ .*content 361x906.*testID=plain' \
  'a plain scroll view still overflows downwards alone'

# The clamp is per axis and per view, so it is checked over every scroll view in
# the dump rather than over the one the eye happens to be on: a view is allowed
# to be bigger inside than out on the axis it asked for, and on no other.
sideways="$(awk '
  match($0, /ScrollView#[0-9]+ \[[-0-9.]+,[-0-9.]+ [0-9.]+x[0-9.]+\]/) {
    line = substr($0, RSTART)
    sub(/.*ScrollView#/, "", line); id = line + 0
    dims = line; sub(/^[0-9]+ \[[-0-9.]+,[-0-9.]+ /, "", dims); sub(/\].*/, "", dims)
    split(dims, size, "x")
    horizontal = ($0 ~ /horizontal=true/)
    if (match($0, /content [0-9.]+x[0-9.]+/)) {
      split(substr($0, RSTART + 8, RLENGTH - 8), content, "x")
      over = horizontal ? (content[2] > size[2] + 0.5) : (content[1] > size[1] + 0.5)
      if (over) printf "    ScrollView#%d: frame %sx%s, content %sx%s\n", id, size[1], size[2], content[1], content[2]
    }
  }' <<<"$OUTPUT")"
if [ -n "$sideways" ]; then
  echo "  FAIL a scroll view is offering an axis it was not asked for:"
  echo "$sideways"
  fail=1
else
  echo "  ok   no scroll view is bigger inside than out across the axis it scrolls on"
fi

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT" | head -40
  exit 1
fi
