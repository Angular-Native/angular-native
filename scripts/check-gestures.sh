#!/usr/bin/env bash
# Gestures and transforms.
#
# What is checked is the whole chain without a device: the recogniser is hooked
# up, the event arrives with its fields, the signal changes, and the transform
# goes out to the host *without* moving the frame.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
cargo an build examples/gestures >/dev/null
OUTPUT="$(cargo run -q -p an-bridge --example headless -- build/bundle/gestures/main.js 6 2>&1)"

fail=0
check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    echo "  ok   $2"
  else
    echo "  FAIL $2"
    fail=1
  fi
}

# The dashes are escaped rather than fenced off with a bare `--`: this `check` is
# a shell function, so a leading `--` is not an end-of-options marker, it is
# simply taken as the pattern — and a pattern of `--` matched every line of the
# dump, so this check passed no matter what the runner printed.
check '\-\- simulated drag' "the drag recogniser is hooked up"
check "translateX=60.00" "the finger's translation reaches the transform"
check "translateY=25.00" "and on both axes"
check "let go at 120 pt/s" "the velocity reaches the template on release"
# The frame is the one the layout gave it, with no transform added on: if
# `translateX` went into the layout, this number would be another one.
check "View#[0-9]+ \[107,90 140x140\]" "transforming moves neither the frame nor the layout"
check "scale=1.00" "the scale starts at 1, not at 0"
# `[style.fontSize]` is the path Angular always lets you write and the one that
# used to be lost on the way: it arrived hyphenated, nobody recognised it, and
# the text was measured with the default font. 23 tall is 18 points; 17 would be
# the default one, which is why the height is what the pattern keys on — the
# width only says how long the label happens to be.
check 'Text#[0-9]+ \[.* [0-9]+x23\]  "swipe here' "a [style.fontSize] reaches the measurement"
# The panel starts 72 tall and the tap takes it to 160. That the transition
# looks smooth is the host's business and is only checked on the device; here
# what is checked is that the new value arrives.
check "View#[0-9]+ \[.* 353x160\]" "the animated panel changes height when tapped"

if [[ $fail -ne 0 ]]; then
  echo "$OUTPUT"
  exit 1
fi
