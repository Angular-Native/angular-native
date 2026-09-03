#!/usr/bin/env bash
# That the layout follows the window on a real Android, and not the screen it
# was started on.
#
# This is outside `check-all.sh` for the same reason as the other two device
# checks: it needs hardware. It is here because an emulator never showed the
# bug and the first run on a real phone did.
#
# What the phone answered before the fix, on a cold start right after the
# install: the whole app laid out into 1080x1079 pixels of a 1080x2340 screen —
# portrait width by portrait width, a square, with the rest of the display left
# black. And after a rotation it stayed 1079 wide inside a 2340-wide window.
#
# The cause was that nothing on Android ever called `setViewport`. The engine
# was handed `DisplayMetrics` once, in `onCreate`, before the container had been
# laid out, and taffy went on laying out for that number for as long as the app
# lived. A rotation, split screen, a foldable being opened and the first frame
# of a cold start are all the same failure.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== rotation on a real Android"

SDK="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
ADB="$SDK/platform-tools/adb"
if [ ! -x "$ADB" ]; then
  echo "  --   there is no adb: the Android SDK is not here"
  exit 0
fi

# A physical phone, not an emulator: an emulator rotates too, but the size it
# was created with is the size it keeps, and this bug needs a window that
# changes under an app that is already running.
SERIAL="${1:-}"
if [ -z "$SERIAL" ]; then
  SERIAL="$("$ADB" devices | awk '$2 == "device" && $1 !~ /^emulator-/ { print $1; exit }')"
fi
if [ -z "$SERIAL" ]; then
  echo "  --   no phone plugged in; connect one and accept USB debugging on it"
  exit 0
fi

adb() { "$ADB" -s "$SERIAL" "$@"; }

if [ "$(adb shell getprop ro.build.characteristics | tr -d '\r')" = "watch" ]; then
  echo "  --   $SERIAL is a watch; this check wants a phone"
  exit 0
fi

cargo an android examples/kitchen --device "$SERIAL" >/dev/null 2>&1 || {
  echo "  FAIL the APK did not reach $SERIAL"
  exit 1
}
sleep 5

# The rotation is forced rather than asked for: a phone lying on a desk with
# auto-rotate on stays portrait however the test wishes otherwise.
AUTO="$(adb shell settings get system accelerometer_rotation | tr -d '\r')"
restore() {
  adb shell settings put system user_rotation 0 >/dev/null 2>&1 || true
  adb shell settings put system accelerometer_rotation "${AUTO:-1}" >/dev/null 2>&1 || true
}
trap restore EXIT
adb shell settings put system accelerometer_rotation 0 >/dev/null

# The root of the app, in pixels, as the system itself reports it.
root_of() {
  adb shell uiautomator dump /sdcard/an-rotation.xml >/dev/null 2>&1
  adb shell cat /sdcard/an-rotation.xml 2>/dev/null | python3 -c '
import re, sys
tree = sys.stdin.read()
groups = re.findall(r"class=\"android.view.ViewGroup\"[^>]*bounds=\"\[0,0\]\[(\d+),(\d+)\]\"", tree)
# The first is the container the Activity sets as its content view, which is
# always the whole window. The one after it is what the framework mounted, and
# that is the number this check is about.
print(" ".join(groups[1]) if len(groups) > 1 else "")
'
}

fail=0
for rotation in 1 0; do
  adb shell settings put system user_rotation "$rotation" >/dev/null
  sleep 4
  window="$(adb shell wm size | tr -d '\r' | awk '{print $3}')"
  read -r width height <<<"$(root_of)"
  if [ -z "${width:-}" ]; then
    echo "  FAIL nothing was mounted after rotating to $rotation"
    fail=1
    continue
  fi
  # In landscape the window is wider than it is tall, and so must the app be.
  # Comparing against the window's own numbers rather than against constants
  # keeps this true on any phone.
  win_w="${window%x*}"
  win_h="${window#*x}"
  [ "$rotation" = "1" ] && { tmp="$win_w"; win_w="$win_h"; win_h="$tmp"; }
  # A point of rounding is expected: the frames are computed in dp and drawn in
  # pixels.
  if [ "$((win_w - width))" -gt 2 ] || [ "$((win_w - width))" -lt -2 ]; then
    echo "  FAIL at rotation $rotation the app is ${width}px wide in a ${win_w}px window"
    fail=1
  elif [ "$((win_h - height))" -gt 2 ] || [ "$((win_h - height))" -lt -2 ]; then
    echo "  FAIL at rotation $rotation the app is ${height}px tall in a ${win_h}px window"
    fail=1
  else
    label="portrait"
    [ "$rotation" = "1" ] && label="landscape"
    echo "  ok   $label: the app is ${width}x${height} in a ${win_w}x${win_h} window"
  fi
done

exit "$fail"
