#!/usr/bin/env bash
# That the Java shell compiles.
#
# `cargo test` and the headless dumps touch no Java: the Android host is 2,600
# lines that until now were only compiled when the APK was built by hand, and a
# whole batch of props could sit there with a type error without anything saying
# so. Building the APK without installing it costs half a minute and compiles
# everything: `aapt2` links the resources, `javac` compiles the shell against
# Material, and `d8` dexes it.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== Android Java shell"
# The output goes to a file and not to /dev/null: with `set -e` and `pipefail`, a
# compilation failure inside a `$(...)` kills the script without printing
# anything and the checker is left silent, which is exactly what must not happen.
LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT
if ! cargo an android examples/controls --no-launch >"$LOG" 2>&1; then
  echo "  FAIL the APK never got built"
  tail -30 "$LOG"
  exit 1
fi
APK="$(tail -1 "$LOG")"
if [ ! -f "$APK" ]; then
  echo "  FAIL the APK never got built"
  tail -30 "$LOG"
  exit 1
fi
echo "  ok   javac compiles AnHost and friends against Material 3"

# The viewport has to follow the window, and only the Java says whether it does.
# `check-rotation-device.sh` proves it on a phone, but it needs one plugged in;
# this is the part that can be asserted anywhere, and it is the part that was
# missing for as long as the bug lived: nothing ever called `setViewport`, so a
# rotation left the app laid out for the width it started with.
ACTIVITY=shells/android/java/dev/angularnative/MainActivity.java
if ! grep -q "addOnLayoutChangeListener" "$ACTIVITY"; then
  echo "  FAIL MainActivity does not watch the container's size"
  exit 1
fi
if ! grep -q "setViewport" "$ACTIVITY"; then
  echo "  FAIL MainActivity never tells the engine the viewport changed"
  exit 1
fi
echo "  ok   the viewport follows the container, so a rotation relays out"
