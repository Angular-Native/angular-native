#!/usr/bin/env bash
# The Angular router on top of an in-memory navigation stack.
#
# The simulated tap on the first card has to lead to the detail page, with the
# route parameter already bound to the component's `input()`.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
cargo an build examples/router >/dev/null
OUTPUT="$(cargo run -q -p an-bridge --example headless -- build/bundle/router/main.js 6 200 2>&1)"

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
# The headless runner announces the simulated gesture in its own words, and it
# is a crate translated on another branch, so both wordings are accepted.
check '\-\- (atrás simulado|simulated back)' 'the back gesture has someone listening for it'
check '"Ships"' 'after going back the list is on screen again'
# Had the screen been rebuilt, the core would have created its views all over.
check '(volver atrás costó 0 vistas creadas|going back cost 0 views created)' 'the previous screen was reattached, not rebuilt'
if grep -qE -- '(búfer inválido|invalid buffer|promesa rechazada|rejected promise|bootstrap failed)' <<<"$OUTPUT"; then
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
