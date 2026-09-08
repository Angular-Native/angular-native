#!/usr/bin/env bash
# The Angular router on top of an in-memory navigation stack.
#
# The simulated tap on the first card has to lead to the detail page, with the
# route parameter already bound to the component's `input()`.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
cargo an build examples/router >/dev/null
# Ten frames and not six. The stack subscribes to `back` only once there is a
# screen underneath, so the subscription arrives a couple of change-detection
# passes after the tap and the unsubscription a couple after the pop: the run
# has to be long enough to see both ends of that.
OUTPUT="$(cargo run -q -p an-bridge --example headless -- build/bundle/router/main.js 10 200 2>&1)"

fail=0
check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    echo "  ok   $2"
  else
    echo "  FAIL $2"
    fail=1
  fi
}

echo "== router"
check 'StackView#[0-9]+' 'the native stack was mounted'
check '\-\- simulated back' 'the back gesture has someone listening for it'
check '"Ships"' 'after going back the list is on screen again'
# The other half of the same contract, and the one that is invisible on a
# screenshot: back at the root belongs to the system. While the app holds the
# subscription Android keeps its OnBackInvokedCallback registered, draws no
# back-to-home preview and answers the press with a `back()` that goes nowhere.
check 'back listeners: none' 'at the root of the stack nothing is listening for back'
# Had the screen been rebuilt, the core would have created its views all over.
check 'going back cost 0 views created' 'the previous screen was reattached, not rebuilt'
if grep -qE -- '(invalid buffer|uncaught rejected promise|bootstrap failed)' <<<"$OUTPUT"; then
  echo "  FAIL there were errors during navigation"
  fail=1
else
  echo "  ok   no protocol errors and no promises left hanging"
fi

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT"
  exit 1
fi
