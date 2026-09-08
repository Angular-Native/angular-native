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
FRAMEWORK="$(mktemp)"
cp "$SOURCE" "$BACKUP"
# Whatever happens, the example is left as it was.
trap 'cp "$BACKUP" "$SOURCE"; rm -f "$BACKUP" "$BEFORE" "$AFTER" "$FRAMEWORK"' EXIT

cargo an build examples/hello-angular >/dev/null
cp build/bundle/hello-angular/main.js "$BEFORE"

# A third bundle, with the framework changed instead of the app, and built
# before the app is touched so the only difference is the framework's.
#
# The change goes into what `ngc` left under `build/js/`, not into
# `packages/platform-native/`: the sources are shared with whoever else is
# working in the tree, and what esbuild reads is this copy anyway. From there on
# it is the real bundler, so the top half really is different and its stamp is
# really computed from it — no faking the signature, which is the one thing that
# cannot happen on a device.
#
# The root node's height is the framework doing something a template cannot: it
# is written by `createRootNode` at bootstrap, so seeing it change is seeing the
# new framework code run from cold.
COMPILED="build/js/hello-angular/packages/platform-native/src/platform.js"
PLATFORM="$(mktemp)"
cp "$COMPILED" "$PLATFORM"
trap 'cp "$BACKUP" "$SOURCE"; cp "$PLATFORM" "$COMPILED"; rm -f "$BACKUP" "$BEFORE" "$AFTER" "$FRAMEWORK" "$PLATFORM"' EXIT
sed -i '' "s/dom.setStyle(root.id, 'height', '100%');/dom.setStyle(root.id, 'height', '50%');/" "$COMPILED"
if ! grep -qF "'50%'" "$COMPILED"; then
  echo "  FAIL createRootNode no longer writes the root's height the way this check patches it"
  exit 1
fi
node scripts/bundle.mjs \
  build/js/hello-angular/examples/hello-angular/src/main.js \
  "$FRAMEWORK" \
  "--alias=@angular-native/platform=$ROOT/build/js/hello-angular/packages/platform-native/src/public-api.js" \
  "--alias=@angular-native/primitives=$ROOT/build/js/hello-angular/packages/primitives/src/public-api.js" \
  >/dev/null
cp "$PLATFORM" "$COMPILED"

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
# hot: there is a single copy inside the interpreter. What happens instead is a
# restart nobody has to ask for: the engine is thrown away, a new one is stood
# up and the bundle just saved is evaluated from cold. The state is gone, and
# that is the price; what must not happen is the developer being left looking at
# the old screen.
echo
echo "== the framework changed"
OTHER="$(run "$FRAMEWORK")"

checkf() {
  if grep -qE -- "$1" <<<"$OTHER"; then
    echo "  ok   $2"
  else
    echo "  FAIL $2"
    fail=1
    OTHER_BAD=1
  fi
}

checkf 'hot reload: no, a restart is needed' 'the stitching is refused instead of lying'
checkf 'restarted: the new bundle is running from cold' 'the restart happens on its own'
# `createRootNode` runs only at bootstrap, so a root half the viewport's height
# is the new framework code having run — not a prop that happened to arrive.
checkf 'View#[0-9]+ \[0,0 393x426\]' "the framework's new code is on screen"
# And the app half came up with it: a restart that mounted nothing would satisfy
# the line above just as well.
checkf '"angular-native"' 'the app came back up on top of it'
if grep -qE -- '"taps: [1-9]' <<<"$OTHER"; then
  echo "  FAIL the component state cannot survive a restart, and it is claiming to"
  fail=1
  OTHER_BAD=1
else
  echo "  ok   the state is gone, which is what a restart costs"
fi

# The one part of `packages/` that no reload can carry. `runtime.js` is the
# prelude and `an-bridge` takes it in with `include_str!`, so it travels inside
# the native binary; the bundle the dev server serves has never held a line of
# it. Saving it used to rebuild the bundle and report a reload that changed
# nothing.
echo
echo "== the prelude is not in the bundle"
if grep -qF 'runFrameCallbacks' packages/runtime/runtime.js &&
  ! grep -qF 'runFrameCallbacks' "$BEFORE"; then
  echo "  ok   what the prelude defines is nowhere in the bundle"
else
  echo "  FAIL the prelude's marker moved; this check is no longer looking at anything"
  fail=1
fi
# So the dev server does not offer a reload for it: it builds the app again and
# puts it back on the device, which is the only thing that carries a new
# prelude.
if grep -qE 'test result: ok\. [1-9]' <<<"$(cargo test -q -p an-cli dev:: 2>&1 || true)"; then
  echo "  ok   the dev server sends a prelude change to a native rebuild"
else
  echo "  FAIL the dev server no longer tells a prelude change apart from a component's"
  fail=1
fi

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
# `$COMPILED` is already back and `build/js/` is regenerated on every build, so
# from here the trap only has the sources and the temporary files to look after.
trap 'cp "$BACKUP" "$SOURCE"; cp "$BACKUP_W" "$SOURCE_W"; rm -f "$BACKUP" "$BEFORE" "$AFTER" "$FRAMEWORK" "$PLATFORM" "$BACKUP_W" "$BEFORE_W" "$AFTER_W"' EXIT

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
#
# The prop is not anchored to the start of the list. They are printed in
# alphabetical order, so anything added to `an-scroll-view` that sorts before
# `refreshing` —`horizontal` did— would break this while proving exactly what
# it is here to prove.
checkw 'ScrollView#[0-9]+ .*props .*refreshing=false' 'the primitives still match as directives'

if [ "$fail" -ne 0 ]; then
  echo
  if [ -n "${WATCH_BAD:-}" ]; then
    echo "$WATCH"
  elif [ -n "${OTHER_BAD:-}" ]; then
    echo "$OTHER"
  else
    echo "$OUTPUT"
  fi
  exit 1
fi
