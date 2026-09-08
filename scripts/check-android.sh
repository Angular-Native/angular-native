#!/usr/bin/env bash
# The Android half of the device-free suite.
#
# It is not a suite of its own: `check-all.sh` calls this, so what "the Android
# half" means is written down once and CI cannot list a different set. It is a
# script rather than four lines inside `check-all.sh` because the two halves
# want different machines — the rest of the suite needs swiftc, a simulator SDK
# and a real `.app`; this needs the NDK and nothing Apple at all — and a CI that
# runs them on one runner each has to be able to name one without the other.
#
# Everything here is device-free. The half that needs a phone or an emulator is
# `check-a11y-device.sh` and friends, and those are in neither script.
set -euo pipefail

# The same safety net `check-all.sh` has, and for the same reason: a sub-script
# killed by `set -e` inside a `$(...)` exits non-zero having printed nothing,
# which reads exactly like nothing having failed.
trap 'echo "  FAIL exited with an error: ${BASH_COMMAND} (check-android.sh line ${LINENO})"' ERR

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

"$ROOT/scripts/check-android-java.sh"

echo
"$ROOT/scripts/check-wearos.sh"

echo
echo "== cross-compilation"
# Android needs the NDK in the environment. It used to come from a committed
# `.cargo/config.toml` holding one machine's paths; it is worked out at build
# time now, and `an env android` is how anything that is not `an android`
# itself — this, a CI job, an editor — gets hold of it.
# An `env` prefix, not a list of exports: most of these names carry the target
# triple with its hyphens, and a shell cannot export one of those.
ANDROID_ENV_ERR="$(mktemp)"
# Only what it printed. `2>&1` would fold cargo's own build warnings into the
# variable, and the whole thing is about to be eval'd.
ANDROID_ENV="$(cargo an env android 2>"$ANDROID_ENV_ERR")" || {
  echo "  FAIL the Android cross-compilation environment could not be worked out"
  sed 's/^/       /' "$ANDROID_ENV_ERR"
  exit 1
}
# x86_64 is here because `--aab` defaults to it: a bundle that stopped
# carrying it would still build, still sign and still upload, and the app would
# simply not be offered to a Chromebook or an emulator.
for target in aarch64-linux-android x86_64-linux-android; do
  # The output is kept, not thrown away: a `2>/dev/null` here cannot tell a
  # crate that does not compile from a toolchain that is not installed, and
  # the two want completely different things done about them.
  LOG="$(mktemp)"
  if eval "$ANDROID_ENV cargo build --quiet -p an-android --target '$target'" >"$LOG" 2>&1; then
    echo "  ok   an-android for $target"
  else
    echo "  FAIL an-android for $target"
    tail -30 "$LOG" | sed 's/^/       /'
    exit 1
  fi
done
