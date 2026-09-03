#!/usr/bin/env bash
# Hot reload: saving a file changes the screen without restarting.
#
# Two bundles of the same example are compiled, one with the template changed,
# and the second is evaluated on top of the first once there is state to lose: a
# counted tap, a timer running and an open `@if`. What this verifies is that the
# change goes in and that the state is still there.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SOURCE="examples/hello-angular/src/app.component.ts"
BACKUP="$(mktemp)"
BEFORE="$(mktemp)"
AFTER="$(mktemp)"
cp "$SOURCE" "$BACKUP"
# Whatever happens, the example is left as it was.
trap 'cp "$BACKUP" "$SOURCE"; rm -f "$BACKUP" "$BEFORE" "$AFTER"' EXIT

cargo an build examples/hello-angular >/dev/null
cp build/bundle/hello-angular/main.js "$BEFORE"

sed -i '' 's/This is an Angular template with signals, running on QuickJS./TEMPLATE CHANGED WHILE HOT./' "$SOURCE"
cargo an build examples/hello-angular >/dev/null
cp build/bundle/hello-angular/main.js "$AFTER"
cp "$BACKUP" "$SOURCE"

run() {
  AN_HOT="$1" cargo run -q -p an-bridge --example headless -- "$BEFORE" 6 2>&1
}

OUTPUT="$(run "$AFTER")"

# If the runner does not even mention the reload, it does not carry the `AN_HOT`
# support: `cargo test` and `cargo run --example` do not always agree on which
# binary of the example is the good one, and the one left over may predate the
# change. It is forced to rebuild once; if it stays the same, that is a real
# failure and dressing it up as a reload that does not work will not do.
if ! grep -qF 'hot reload' <<<"$OUTPUT"; then
  touch crates/an-bridge/examples/headless.rs
  cargo build -q -p an-bridge --example headless
  OUTPUT="$(run "$AFTER")"
fi
if ! grep -qF 'hot reload' <<<"$OUTPUT"; then
  echo "  FAIL the headless runner has no AN_HOT support, not even after a rebuild"
  exit 1
fi

fail=0
check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    echo "  ok   $2"
  else
    echo "  FAIL $2"
    fail=1
  fi
}

echo "== hot reload"
check 'hot reload: yes' 'the new bundle was stitched onto the one already running'
check 'TEMPLATE CHANGED WHILE HOT' 'the new template is on screen'
check '"taps: 1 \(last at 40, 20\)"' 'the component state survived'
check '"seconds running: 5"' 'the timer kept running, it did not go back to zero'
check 'The @if came in at 3 seconds' 'what had already unfolded is still unfolded'
if grep -qE -- 'This is an Angular template' <<<"$OUTPUT"; then
  echo "  FAIL the old screen stayed mounted underneath the new one"
  fail=1
else
  echo "  ok   the old screen was unmounted"
fi

# The top half of the bundle —Angular and the framework— cannot be reloaded
# hot: there is a single copy inside the interpreter. When it changes, the
# honest thing is to ask for a restart, and that is what is checked here by
# faking the signature.
sed 's/globalThis.__anVendor !== "/globalThis.__anVendor !== "x/' "$AFTER" >"$AFTER.other"
OTHER="$(run "$AFTER.other")"
if grep -qF -- 'hot reload: no' <<<"$OTHER"; then
  echo "  ok   if the framework changes a restart is asked for instead of lying"
else
  echo "  FAIL changing the framework should force a restart"
  fail=1
fi
rm -f "$AFTER.other"

# And the same exercise with a template that uses a component —not a
# directive—, because they are two different things and only one of them showed.
#
# `hello-angular` is all primitives, and a primitive is a directive on an
# element the core mounts either way: if the reload leaves the template with no
# directives, `[backgroundColor]` still arrives as a property and the screen does
# not change. What gives the bug away is a component that does something a
# property cannot do —write styles from its host and project children—, and that
# is `an-safe-area`, which only appears in `hello-wear`.
SOURCE_W="examples/hello-wear/src/app.component.ts"
BACKUP_W="$(mktemp)"
BEFORE_W="$(mktemp)"
AFTER_W="$(mktemp)"
cp "$SOURCE_W" "$BACKUP_W"
trap 'cp "$BACKUP" "$SOURCE"; cp "$BACKUP_W" "$SOURCE_W"; rm -f "$BACKUP" "$BEFORE" "$AFTER" "$BACKUP_W" "$BEFORE_W" "$AFTER_W"' EXIT

cargo an build examples/hello-wear >/dev/null
cp build/bundle/hello-wear/main.js "$BEFORE_W"
sed -i '' 's/Turn the crown: {{ offset() }} pt\./WATCH CHANGED WHILE HOT./' "$SOURCE_W"
cargo an build examples/hello-wear >/dev/null
cp build/bundle/hello-wear/main.js "$AFTER_W"
cp "$BACKUP_W" "$SOURCE_W"

# On the emulator's watch face, so the tree measurements are the watch's own.
WATCH="$(AN_VIEWPORT=227x227 AN_HOT="$AFTER_W" \
  cargo run -q -p an-bridge --example headless -- "$BEFORE_W" 6 2>&1)"

checkw() {
  if grep -qE -- "$1" <<<"$WATCH"; then
    echo "  ok   $2"
  else
    echo "  FAIL $2"
    fail=1
    WATCH_BAD=1
  fi
}

checkw 'hot reload: yes' 'the watch is stitched hot as well'
checkw 'WATCH CHANGED WHILE HOT' "the watch's new template is on screen"
# The safe area is a component: its styles are written by its host, not by the
# template. Without it the scroll view does not grow and stays at 227x0, which is
# a black screen without a single error.
checkw 'ScrollView#[0-9]+ \[0,0 227x227\]' "the safe area host's styles are still applied"
# And the primitives' inputs are still inputs of a directive and not loose
# properties that happen to end up in the same place.
checkw 'ScrollView#[0-9]+ .*props refreshing=false' 'the primitives still match as directives'

if [ "$fail" -ne 0 ]; then
  echo
  if [ -n "${WATCH_BAD:-}" ]; then
    echo "$WATCH"
  else
    echo "$OUTPUT"
  fi
  exit 1
fi
