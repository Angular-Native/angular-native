#!/usr/bin/env bash
# Native modules: that every host has the one module the framework promises, and
# that the ones it has not are turned down out loud.
#
# `Device` is not a plugin. It is not an npm package and nobody declares it: it
# is compiled into every host, and `packages/platform-native` offers it to any
# app on any platform with no ceremony at all. Which means a host that does not
# register it does not fail to build and does not warn about anything — the app
# calls `Device.info()`, the promise rejects, and the screen shows whatever the
# `catch` decided to show. That went unnoticed on two of the seven platforms.
#
# So there are three things here, and they are three because they can each break
# without the others noticing:
#
#   1. Every value of `NativePlatform` is produced by some host. A value nobody
#      ever answers is a lie in a type the apps switch on.
#   2. macOS says who it is, read off the window of a running app. It is the one
#      platform where that can be done without a simulator.
#   3. The watch's module, over the real registry, from `cargo test`.
#
# And the fourth, which is about what is *not* there: neither of the two hosts
# without a plugin registry may reject an unknown module with nothing but its
# name. A name that is not there because this host cannot ever have it is not the
# same thing as a typo, and whoever reads the message has no other way to tell.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== native modules"

fail=0
check() { # <0 if good, 1 if bad> <what was being checked>
  if [ "$1" -eq 0 ]; then
    echo "  ok   $2"
  else
    echo "  FAIL $2"
    fail=1
  fi
}

# 1. Every platform in the type is somebody's answer.
#
# The values come out of the type itself, so adding one to `NativePlatform`
# without a host that produces it is caught here rather than by an app's `switch`
# falling through months later.
CONTRACT="packages/platform-native/src/native-modules.ts"
IOS_DEVICE="crates/an-ios/src/modules/device.rs"
MAC_DEVICE="crates/an-macos/src/modules/device.rs"
WATCH_DEVICE="crates/an-watch/src/modules.rs"
ANDROID_HOST="shells/android/java/dev/angularnative/AnHost.java"

PLATFORMS="$(awk '/^export type NativePlatform =/,/^$/' "$CONTRACT" \
  | grep -oE "'[a-z]+'" | tr -d "'" | sort -u)"
if [ -z "$PLATFORMS" ]; then
  echo "  FAIL the NativePlatform union could not be read from $CONTRACT"
  exit 1
fi

# `AnHost.deviceInfo()` builds the JSON by hand, so the platform is looked for
# inside that method and not in the whole 3,000-line file, where `android`
# appears on nearly every line.
ANDROID_INFO="$(awk '/public String deviceInfo\(\)/,/^    \}/' "$ANDROID_HOST")"

# The one value with no producer. Wear OS reports itself as a phone: `deviceInfo`
# hard-codes the platform, so an app on a watch cannot tell it is on one. It is a
# real hole and it is named here rather than quietly left out of the loop; take
# the word off this line and the check below turns it into a failure.
KNOWN_MISSING="wearos"

produced_by() {
  case "$1" in
    ios | tvos | visionos) grep -qE "PLATFORM: &str = \"$1\"" "$IOS_DEVICE" ;;
    macos) grep -qE "platform: \"macos\"" "$MAC_DEVICE" ;;
    watchos) grep -qE "PLATFORM: &str = \"watchos\"" "$WATCH_DEVICE" ;;
    android | wearos) grep -qw "$1" <<<"$ANDROID_INFO" ;;
    *) return 1 ;;
  esac
}

covered=0
for platform in $PLATFORMS; do
  if produced_by "$platform"; then
    covered=$((covered + 1))
  elif grep -qw "$platform" <<<"$KNOWN_MISSING"; then
    echo "  warn no host ever answers '$platform': see the Wear OS gap in"
    echo "       docs-site/src/content/docs/platforms/wearos.md"
  else
    echo "  FAIL '$platform' is in NativePlatform and no host produces it"
    fail=1
  fi
done
echo "  ok   $covered of the $(wc -w <<<"$PLATFORMS" | tr -d ' ') values of NativePlatform are answered by a host"

# 2. And that neither of the two hosts without plugins rejects a module with
#    nothing but its name.
for crate in an-macos an-watch; do
  grep -q 'ABSENT_NOTE' "crates/$crate/src/ffi.rs" && r=0 || r=1
  check $r "$crate says why a module it does not have is not there"
done

# 3. The watch's module, over the real registry: registered under `device`,
#    answering `info`, and with the platform put in by Rust and not by the shell.
if cargo test --quiet -p an-watch modules:: >/dev/null 2>&1; then
  echo "  ok   the watch's device module answers through the module registry"
else
  echo "  FAIL the an-watch module tests do not pass"
  cargo test -p an-watch modules:: 2>&1 | tail -20
  fail=1
fi

# The four fields the watch cannot read from Rust come from the shell, which is
# the same arrangement Android has with `AnHost.deviceInfo()`. If the shell stops
# handing them over, the module answers nothing and says so — but it is better to
# find out here.
grep -q 'WKInterfaceDevice' shells/watchos/Sources/AnRuntime.swift && r=0 || r=1
check $r "the watch shell reads the device from WatchKit and hands it over"
grep -q 'device_json' crates/an-watch/include/angular_native_watch.h && r=0 || r=1
check $r "and the C header says so, which is what the shell compiles against"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "  --   the rest is skipped: the macOS host only builds on a Mac"
  exit "$fail"
fi

# 4. macOS, on a running window.
#
# `examples/kitchen` is the one that calls `Device.info()` and paints the answer,
# so what is checked is the string the app actually shows — not that the module
# is in a list. Before this existed it showed «no device data: …», which is the
# `catch` doing its job over a promise nobody could fulfil.
BUILD_LOG="$(mktemp)"
RUN_LOG="$(mktemp)"
trap 'rm -f "$BUILD_LOG" "$RUN_LOG"' EXIT

if ! cargo an macos examples/kitchen --no-launch >"$BUILD_LOG" 2>&1; then
  echo "  FAIL the kitchen example does not build for macOS"
  tail -30 "$BUILD_LOG"
  exit 1
fi

BIN="$ROOT/build/macos/AngularNativeMac.app/Contents/MacOS/AngularNativeMac"
AN_SCREENSHOT="$ROOT/build/macos/device.png" AN_SCREENSHOT_FRAMES=120 AN_DUMP_TEXT=1 \
  "$BIN" >"$RUN_LOG" 2>&1 || true

if grep -qE '\[text\] macos [0-9]+\.[0-9]+' "$RUN_LOG"; then
  echo "  ok   $(sed -n "s/.*\[text\] \(macos .*\)$/\1/p" "$RUN_LOG" | tail -1) — Device.info() answers on the desktop"
else
  echo "  FAIL Device.info() does not answer on macOS"
  grep -E '\[text\]' "$RUN_LOG" | sed 's/^/       /' | tail -10
  fail=1
fi

# And that it is not the `catch` painting an apology.
if grep -q 'no device data' "$RUN_LOG"; then
  echo "  FAIL the promise rejected: $(grep -o 'no device data.*' "$RUN_LOG" | tail -1)"
  fail=1
else
  echo "  ok   the promise resolved, it was not caught"
fi

if [ "$fail" -ne 0 ]; then
  echo
  cat "$RUN_LOG"
  exit 1
fi
