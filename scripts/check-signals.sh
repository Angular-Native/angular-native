#!/usr/bin/env bash
# No decorators: inputs, outputs and queries all go through signals.
#
# It is not a matter of taste. An `@Input() set` runs at the exact moment Angular
# writes the input, so the order of the writes depends on the order of the
# template's bindings; a signal is read when somebody reads it, and what sends to
# the core is an `effect`. Mixing the two in the same tree is asking for
# something to arrive in a different order depending on who wrote it.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== signals"
fail=0
forbidden() {
  # The call is searched for, not the word: `@Input` appears in the comments
  # explaining why it is no longer used.
  found="$(grep -rn --include='*.ts' -- "$1" packages examples || true)"
  if [ -n "$found" ]; then
    echo "  FAIL $2"
    echo "$found" | sed 's/^/         /'
    fail=1
  else
    echo "  ok   $2"
  fi
}

forbidden '@Input(' 'no input with a decorator; they go through input()'
forbidden '@Output(' 'no output with a decorator; they go through output()'
forbidden 'new EventEmitter' 'no EventEmitter; the outputs are output()'
forbidden '@ViewChild\|@ViewChildren\|@ContentChild\|@ContentChildren' \
  'no query with a decorator; they go through viewChild() and contentChild()'
forbidden '@HostBinding\|@HostListener' \
  'no HostBinding and no HostListener; they go in the decorator host'

exit "$fail"
