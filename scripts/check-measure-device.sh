#!/usr/bin/env bash
# That the controls really do come out with a size on a real Android.
#
# `check-measure.sh` reads the sources and says that every name the core asks
# for has a case in the host. That is the invariant, and it is what would have
# caught this bug, but it is still only a promise about text. This one installs
# `examples/measure` —where not one control is given a width or a height by the
# template, so the host's measurement is the only thing deciding— and asks the
# system what it ended up drawing. `uiautomator dump` serialises the
# `AccessibilityNodeInfo` tree with the frame of every node in it.
#
# What that dump answered before the fix is worth writing down, because it is
# what the bug looked like from outside: only `an-icon` and a label came out.
# The navigation bar, the search bar, the segmented control, the dropdown, the
# stepper and the date picker were not in the tree at all. Not with the wrong
# size — not there. A view of 0x0 has nothing to report.
#
# It is out of `check-all.sh` on purpose, like `check-a11y-device.sh`: it
# builds an APK, installs it and waits for a screen, and it needs a device.
#
#   ./scripts/check-measure-device.sh                # picks the only device
#   ./scripts/check-measure-device.sh emulator-5554  # or names one
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== controls measured on a device"

SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
ADB="$SDK/platform-tools/adb"
if [ ! -x "$ADB" ]; then
  echo "  FAIL no adb in $SDK/platform-tools"
  exit 1
fi

# Which device. A phone plugged in and an emulator running at the same time is
# the normal state of this machine, and an `adb` with no `-s` refuses to choose.
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

# A watch does not mount six of these —`unsupported()` says why for each— and
# what it mounts instead is the marker, so the frames would be the marker's.
if "$ADB" -s "$SERIAL" shell getprop ro.build.characteristics | grep -q watch; then
  echo "  FAIL $SERIAL is a watch, and it does not mount half of what this screen asks for."
  echo "         Run this on a phone."
  exit 1
fi
echo "  ok   device $SERIAL"

# The output goes to a file and not to /dev/null: with `set -e` and `pipefail`,
# a build failure inside a `$(...)` kills the script without printing anything
# and the checker goes quiet, which is the one thing it must not do.
LOG="$(mktemp)"
DUMP="$(mktemp)"
trap 'rm -f "$LOG" "$DUMP"' EXIT

if ! cargo an android examples/measure --no-launch >"$LOG" 2>&1; then
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
echo "  ok   an android builds examples/measure"

if ! "$ADB" -s "$SERIAL" install -r "$APK" >"$LOG" 2>&1; then
  echo "  FAIL the APK did not install"
  tail -10 "$LOG"
  exit 1
fi
"$ADB" -s "$SERIAL" shell am force-stop dev.angularnative
"$ADB" -s "$SERIAL" shell am start -n dev.angularnative/.MainActivity >/dev/null

# What is waited for is the thing the check needs, not a number of seconds: the
# icon, which is the one control that was coming out before the fix and so
# cannot be mistaken for the fix working.
FOUND=""
for _ in $(seq 1 40); do
  if "$ADB" -s "$SERIAL" shell uiautomator dump /sdcard/an-measure.xml >/dev/null 2>&1 &&
     "$ADB" -s "$SERIAL" shell cat /sdcard/an-measure.xml 2>/dev/null | grep -q 'the icon'; then
    FOUND="yes"
    break
  fi
  sleep 1
done
if [ -z "$FOUND" ]; then
  echo "  FAIL the screen never came up: no node named 'the icon' after 40 s"
  "$ADB" -s "$SERIAL" logcat -d -s angular-native:* AndroidRuntime:E | tail -30
  exit 1
fi
"$ADB" -s "$SERIAL" shell cat /sdcard/an-measure.xml > "$DUMP"
echo "  ok   the system reports its tree"

python3 - "$DUMP" <<'PY'
import re, sys

dump = open(sys.argv[1]).read()

# Every control on that screen carries its `[accessibilityLabel]`, which is what
# a node comes out under.
WANTED = [
    'the navigation bar',
    'the search bar',
    'the segmented control',
    'the dropdown',
    'the stepper',
    'the date picker',
    'the icon',
]

frames = {}
for node in re.finditer(r'<node[^>]*>', dump):
    text = node.group(0)
    label = re.search(r'content-desc="([^"]*)"', text)
    bounds = re.search(r'bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"', text)
    if not label or not label.group(1) or not bounds:
        continue
    left, top, right, bottom = (int(n) for n in bounds.groups())
    frames[label.group(1)] = (right - left, bottom - top)

failures = 0
for label in WANTED:
    frame = frames.get(label)
    if frame is None:
        # Not "the wrong size": not in the tree. That is what 0x0 looks like
        # from up here, and it is exactly what six of these did before.
        print(f'  FAIL {label} is not in the tree: it was laid out with no size at all')
        failures += 1
    elif frame[0] <= 0 or frame[1] <= 0:
        print(f'  FAIL {label} came out {frame[0]}x{frame[1]} px')
        failures += 1
    else:
        print(f'  ok   {label} is on screen at {frame[0]}x{frame[1]} px')

sys.exit(1 if failures else 0)
PY

# And that the host did not have to fall back on anything. A control it cannot
# measure now says so; if that line is in the log, the screen above came out by
# luck and not because it was measured.
if "$ADB" -s "$SERIAL" logcat -d -s angular-native:E | grep -q 'has no measurement in this host'; then
  echo "  FAIL the host could not measure something it was asked about:"
  "$ADB" -s "$SERIAL" logcat -d -s angular-native:E | grep 'has no measurement' | tail -5
  exit 1
fi
echo "  ok   the host was not asked for a control it cannot measure"
