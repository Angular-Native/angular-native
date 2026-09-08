#!/usr/bin/env bash
# Accessibility on a real Android, seen through the system's own eyes.
#
# Everything else in `scripts/` reads what we wrote. This reads what the
# platform built out of it: `uiautomator dump` serialises the
# `AccessibilityNodeInfo` tree, which is the same tree TalkBack walks. A label
# that shows up there is a label that arrived.
#
# It is out of `check-all.sh` on purpose. It builds an APK, installs it and
# waits for a screen — around two minutes with a warm emulator — and it needs a
# device, which a machine running the checks may not have. The half that costs
# nothing is `check-a11y.sh`, and that one is hooked up.
#
#   ./scripts/check-a11y-device.sh                # picks the only device
#   ./scripts/check-a11y-device.sh emulator-5554  # or names one
#
# There is no emulator by default; the phone one used here is created with:
#
#   sdkmanager "system-images;android-36;google_apis;arm64-v8a"
#   avdmanager create avd -n an-test \
#     -k "system-images;android-36;google_apis;arm64-v8a" -d pixel_6
#   emulator -avd an-test -port 5554 -no-boot-anim
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== accessibility on a device"

SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
ADB="$SDK/platform-tools/adb"
if [ ! -x "$ADB" ]; then
  echo "  FAIL no adb in $SDK/platform-tools"
  exit 1
fi

# Which device. A phone plugged in and an emulator running at the same time is
# the normal state of this machine, and an `adb` with no `-s` refuses to choose
# — or worse, an unauthorised phone is picked and everything fails later with
# an error that says nothing about accessibility.
SERIAL="${1:-}"
if [ -z "$SERIAL" ]; then
  READY="$("$ADB" devices | awk '$2 == "device" { print $1 }')"
  COUNT="$(printf '%s\n' "$READY" | grep -c . || true)"
  if [ "$COUNT" -eq 0 ]; then
    echo "  FAIL no device ready: start an emulator or plug a phone in"
    "$ADB" devices
    exit 1
  fi
  if [ "$COUNT" -gt 1 ]; then
    echo "  FAIL more than one device ready; name the one to use:"
    printf '%s\n' "$READY" | sed 's/^/         /'
    exit 1
  fi
  SERIAL="$READY"
fi

# A watch is not the device for this. Wear OS runs this very host and the same
# code path — the accessibility work is not conditional on the shape of the
# screen — but 227 round points do not fit the rows this template lays out, and
# what is off screen is not in the accessibility tree either. The check would
# fail on the screen size and read as if the labels had not arrived.
if grep -q watch <<<"$("$ADB" -s "$SERIAL" shell getprop ro.build.characteristics || true)"; then
  echo "  FAIL $SERIAL is a watch, and examples/a11y does not fit on one."
  echo "         Wear OS inherits all of this: same AnHost, same AnAccessibility,"
  echo "         same delegate. Run this on a phone."
  exit 1
fi
echo "  ok   device $SERIAL"

# The output goes to a file and not to /dev/null: with `set -e` and `pipefail`,
# a build failure inside a `$(...)` kills the script without printing anything,
# and the checker itself goes quiet, which is the one thing it must not do.
LOG="$(mktemp)"
DUMP_FULL="$(mktemp)"
DUMP_COMPRESSED="$(mktemp)"
trap 'rm -f "$LOG" "$DUMP_FULL" "$DUMP_COMPRESSED"' EXIT

# `AN_ABI` names the architecture the APK has to carry. The default repeats
# `an android`'s own — arm64-v8a, which is the phone plugged in and the emulator
# on an Apple-silicon Mac — and it is written out rather than left off because
# `--abi` takes a value and there is no way to pass "whatever you were going to
# pick". It exists because the emulator on an x86_64 Linux CI runner is the one
# device that is not arm64, and an arm64-only APK does not install on it: `adb`
# refuses with INSTALL_FAILED_NO_MATCHING_ABIS, which says nothing about
# accessibility.
if ! cargo an android examples/a11y --no-launch --abi "${AN_ABI:-arm64-v8a}" >"$LOG" 2>&1; then
  echo "  FAIL the APK did not build"
  tail -30 "$LOG"
  exit 1
fi
APK="$(tail -1 "$LOG")"
if [ ! -f "$APK" ]; then
  echo "  FAIL the APK did not build"
  tail -30 "$LOG"
  exit 1
fi
echo "  ok   an android builds examples/a11y"

if ! "$ADB" -s "$SERIAL" install -r "$APK" >"$LOG" 2>&1; then
  echo "  FAIL the APK did not install"
  tail -10 "$LOG"
  exit 1
fi
"$ADB" -s "$SERIAL" shell am force-stop dev.angularnative
"$ADB" -s "$SERIAL" shell am start -n dev.angularnative/.MainActivity >/dev/null

# The dump has to wait for the screen. Waiting a fixed number of seconds is
# what makes a check like this flaky, so what is waited for is the thing the
# check needs: a node the template asked for. If it never turns up, what gets
# printed is the app's own log, which is where the reason will be.
FOUND=""
for _ in $(seq 1 40); do
  if "$ADB" -s "$SERIAL" shell uiautomator dump /sdcard/an-a11y.xml >/dev/null 2>&1 &&
     grep -q 'Save the draft' <<<"$("$ADB" -s "$SERIAL" shell cat /sdcard/an-a11y.xml 2>/dev/null || true)"; then
    FOUND="yes"
    break
  fi
  sleep 1
done
if [ -z "$FOUND" ]; then
  echo "  FAIL the screen never came up: no node named 'Save the draft' after 40 s"
  "$ADB" -s "$SERIAL" logcat -d -s angular-native:* AndroidRuntime:E | tail -30
  exit 1
fi

"$ADB" -s "$SERIAL" shell cat /sdcard/an-a11y.xml > "$DUMP_FULL"
"$ADB" -s "$SERIAL" shell uiautomator dump --compressed /sdcard/an-a11y-c.xml >/dev/null
"$ADB" -s "$SERIAL" shell cat /sdcard/an-a11y-c.xml > "$DUMP_COMPRESSED"
echo "  ok   the system reports its accessibility tree, both trees dumped"

python3 "$ROOT/scripts/check-a11y-dump.py" "$ROOT" "$DUMP_FULL" "$DUMP_COMPRESSED"

# The log is read too, because part of the contract is what the host refuses to
# do. `expanded` on something that cannot be pressed, or a role the host does
# not know, have to end up in the log: that is the difference between a
# decision and a silent drop.
if grep -q 'accessibility' <<<"$("$ADB" -s "$SERIAL" logcat -d -s angular-native:E || true)"; then
  echo "  FAIL the host complained about something the template asked for:"
  "$ADB" -s "$SERIAL" logcat -d -s angular-native:E | grep 'accessibility' | tail -5
  exit 1
fi
echo "  ok   the host had nothing to complain about in this template"
