#!/usr/bin/env bash
# Everything that can be verified without a device.
set -euo pipefail

# A safety net: say what fell over.
#
# A sub-script can die without ever printing its own `FAIL` —it is enough for
# `set -e` to kill it inside a `$(...)` that was covering its output—, and then
# this exited with 1 and without a single line to read, which is the worst
# possible way of failing: it looks as though nothing failed. What fell over is
# said here even when nothing was said there.
trap 'echo "  FAIL exited with an error: ${BASH_COMMAND} (check-all.sh line ${LINENO})"' ERR

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== duplicated lists"
"$ROOT/scripts/check-styles.sh"
"$ROOT/scripts/check-kinds.sh"

echo
"$ROOT/scripts/check-signals.sh"

echo
echo "== props that reach both hosts"
"$ROOT/scripts/check-wrapper.sh"

echo
echo "== Rust core"
cargo test --quiet 2>&1 | tail -1

"$ROOT/scripts/check-angular.sh"
"$ROOT/scripts/check-list.sh"
"$ROOT/scripts/check-router.sh"
"$ROOT/scripts/check-controls.sh"
"$ROOT/scripts/check-gestures.sh"
"$ROOT/scripts/check-pickers.sh"
"$ROOT/scripts/check-web.sh"
"$ROOT/scripts/check-media.sh"
"$ROOT/scripts/check-hot.sh"
"$ROOT/scripts/check-watchos.sh"
"$ROOT/scripts/check-tvos.sh"
"$ROOT/scripts/check-visionos.sh"

echo
"$ROOT/scripts/check-macos.sh"

echo
# The development loop, end to end. It goes here and not next to `check-hot.sh`
# because it needs the `.app` the line above just built: macOS is the one
# platform where the whole round trip —serve, build, launch, save, refresh— can
# be watched without a simulator or a person.
"$ROOT/scripts/check-dev-macos.sh"

echo
"$ROOT/scripts/check-accessibility.sh"

"$ROOT/scripts/check-external.sh"

echo
"$ROOT/scripts/check-plugins.sh"

echo
# What is not a plugin: the module every host is supposed to have compiled in.
"$ROOT/scripts/check-modules.sh"

echo
"$ROOT/scripts/check-permissions.sh"

echo
"$ROOT/scripts/check-secrets.sh"

echo
"$ROOT/scripts/check-android-java.sh"

echo
"$ROOT/scripts/check-wearos.sh"

echo
# Only the half that costs nothing. The other one — build the APK, install it
# and ask the system for its accessibility tree — is `check-a11y-device.sh`,
# and it needs a device.
"$ROOT/scripts/check-a11y.sh"

echo
echo "== cross-compilation"
for target in aarch64-apple-ios-sim aarch64-linux-android; do
  case "$target" in
    *ios*) crate=an-ios ;;
    *) crate=an-android ;;
  esac
  if cargo build --quiet -p "$crate" --target "$target" 2>/dev/null; then
    echo "  ok   $crate for $target"
  else
    echo "  FAIL $crate for $target"
    exit 1
  fi
done
