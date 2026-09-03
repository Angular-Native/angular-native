#!/usr/bin/env bash
# The keyboard, on a running Android, measured rather than believed.
#
# `an-safe-area` reports the keyboard in its bottom inset, and the only way to
# know that a form actually got out of the way is to focus a field at the bottom
# of one and ask the system where that field ended up. `uiautomator dump`
# serialises the `AccessibilityNodeInfo` tree with real bounds in pixels, so the
# question can be asked exactly: did the last field's bottom edge come up by the
# height the host said the keyboard was?
#
# It is out of `check-all.sh` for the same reason `check-a11y-device.sh` is: it
# builds an APK, installs it and waits for a screen, and it needs a device.
#
#   ./scripts/check-keyboard-device.sh                # picks the only device
#   ./scripts/check-keyboard-device.sh emulator-5554  # or names one
#
# **There is no iOS half, and that is not an oversight.** The same check on the
# simulator needs one thing this Mac cannot do: put a tap on a field. `simctl`
# has no verb for it, the Simulator publishes no accessibility tree for its
# device windows to drive instead, and locating its window to click it needs the
# Screen Recording permission. What the iOS host does is in
# `crates/an-ios/src/host.rs`; what checks it is a person with a simulator open.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== the keyboard on a device"

fail=0
check() { # <0 ok, 1 bad> <what was being checked>
  if [ "$1" -eq 0 ]; then
    echo "  ok   $2"
  else
    echo "  FAIL $2"
    fail=1
  fi
}

SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
ADB="$SDK/platform-tools/adb"
if [ ! -x "$ADB" ]; then
  echo "  FAIL no adb in $SDK/platform-tools"
  exit 1
fi

# Which device. A phone plugged in and an emulator running at once is the normal
# state of this machine, and an `adb` with no `-s` refuses to choose.
SERIAL="${1:-}"
if [ -z "$SERIAL" ]; then
  READY="$("$ADB" devices | awk '$2 == "device" { print $1 }')"
  COUNT="$(printf '%s\n' "$READY" | grep -c . || true)"
  if [ "$COUNT" -eq 0 ]; then
    echo "  FAIL no device ready: start an emulator or plug a phone in"
    exit 1
  fi
  if [ "$COUNT" -gt 1 ]; then
    echo "  FAIL more than one device ready; name the one to use:"
    printf '%s\n' "$READY" | sed 's/^/         /'
    exit 1
  fi
  SERIAL="$READY"
fi

# A watch has no keyboard of this kind: Wear OS answers a text field with its own
# full-screen input, so there is no inset to report and nothing here to measure.
if "$ADB" -s "$SERIAL" shell getprop ro.build.characteristics | grep -q watch; then
  echo "  --   skipped: $SERIAL is a watch, and a watch answers a text field with a"
  echo "       screen of its own rather than with an inset over this one"
  exit 0
fi

# The IME inset and its animation callback are API 30. Below that the host
# reports nothing and says so; there is no point measuring for it.
API="$("$ADB" -s "$SERIAL" shell getprop ro.build.version.sdk | tr -d '\r')"
if [ "$API" -lt 30 ]; then
  echo "  --   skipped: API $API. WindowInsets.Type.ime() and"
  echo "       WindowInsetsAnimation.Callback both arrived in 30, and AnHost says so."
  exit 0
fi

LOG="$(mktemp)"
DUMP="$(mktemp)"
trap 'rm -f "$LOG" "$DUMP"' EXIT

if ! cargo an android examples/keyboard --no-launch >"$LOG" 2>&1; then
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
echo "  ok   an android builds examples/keyboard"

if ! "$ADB" -s "$SERIAL" install -r "$APK" >"$LOG" 2>&1; then
  echo "  FAIL the APK did not install"
  tail -10 "$LOG"
  exit 1
fi
# A phone that went to sleep between two runs shows the lock screen and not the
# form, and the failure then reads as "the form never came up", which is a lie
# about the code. It is woken and unlocked first.
"$ADB" -s "$SERIAL" shell input keyevent KEYCODE_WAKEUP >/dev/null 2>&1 || true
"$ADB" -s "$SERIAL" shell wm dismiss-keyguard >/dev/null 2>&1 || true
# And the notification shade, which a stray drag in an earlier run leaves open
# over everything and which `uiautomator` then dumps instead of the app.
"$ADB" -s "$SERIAL" shell cmd statusbar collapse >/dev/null 2>&1 || true
"$ADB" -s "$SERIAL" shell am force-stop dev.angularnative
"$ADB" -s "$SERIAL" shell am start -n dev.angularnative/.MainActivity >/dev/null

# `wm density` prints the physical density and, when there is one, an override
# under it. The last line is the one in force, and it is the one the host
# divided by.
DENSITY="$("$ADB" -s "$SERIAL" shell wm density | sed -n 's/.*density: *//p' | tail -1 | tr -d '\r')"

# One reading: the reported bottom inset and the last field's bottom edge, in
# pixels. The inset is on screen because a check has to be able to tell "the
# keyboard was never reported" from "it was reported and the layout ignored it".
read_state() { # -> "<inset dp> <field bottom px>"
  "$ADB" -s "$SERIAL" shell uiautomator dump /sdcard/an-keyboard.xml >/dev/null 2>&1 || return 1
  "$ADB" -s "$SERIAL" shell cat /sdcard/an-keyboard.xml >"$DUMP" 2>/dev/null || return 1
  python3 - "$DUMP" <<'PY'
import re, sys
xml = open(sys.argv[1], encoding='utf-8', errors='replace').read()
inset = re.search(r'bottom inset (\d+)', xml)
field = re.search(r'content-desc="password"[^>]*?bounds="\[\d+,\d+\]\[\d+,(\d+)\]"', xml)
if not inset or not field:
    sys.exit(1)
print(inset.group(1), field.group(1))
PY
}

# The screen has to be there before there is anything to measure. What is waited
# for is the thing the check needs, not a number of seconds.
STATE=""
for _ in $(seq 1 40); do
  if STATE="$(read_state 2>/dev/null)"; then
    break
  fi
  sleep 1
done
if [ -z "$STATE" ]; then
  echo "  FAIL the form never came up: no field labelled 'password' after 40 s"
  "$ADB" -s "$SERIAL" logcat -d -s angular-native:* AndroidRuntime:E | tail -30
  exit 1
fi
REST_INSET="${STATE% *}"
REST_BOTTOM="${STATE#* }"
echo "  ok   the form is up, with its last field at $REST_BOTTOM px and the bottom"
echo "       inset reported as $REST_INSET (the navigation bar)"

# Focus the last field: a tap in the middle of it, which is what a finger does.
TAP_Y=$((REST_BOTTOM - 40))
TAP_X="$("$ADB" -s "$SERIAL" shell wm size | sed 's/.*: //' | cut -dx -f1 | tr -d '\r')"
TAP_X=$((TAP_X / 2))
"$ADB" -s "$SERIAL" shell input tap "$TAP_X" "$TAP_Y"
sleep 3

UP="$(read_state || true)"
if [ -z "$UP" ]; then
  echo "  FAIL the tree could not be read with the keyboard up"
  exit 1
fi
UP_INSET="${UP% *}"
UP_BOTTOM="${UP#* }"

[ "$UP_INSET" -gt "$REST_INSET" ] && r=0 || r=1
check $r "focusing the field reports a bottom inset of $UP_INSET, up from $REST_INSET"

# And the one that matters: the field is above the keyboard, not under it. The
# host says the keyboard covers UP_INSET points of the screen; the field's bottom
# edge has to have come up by that much, give or take a pixel of rounding.
MOVED=$((REST_BOTTOM - UP_BOTTOM))
EXPECTED="$(python3 -c "print(round(($UP_INSET - $REST_INSET) * $DENSITY / 160))")"
SLACK=$(( EXPECTED / 20 + 4 ))
[ "$MOVED" -ge $((EXPECTED - SLACK)) ] && [ "$MOVED" -le $((EXPECTED + SLACK)) ] && r=0 || r=1
check $r "and the field came up $MOVED px, which is the ${EXPECTED} px the keyboard covers"
if [ "$r" -ne 0 ]; then
  echo "       the field's bottom went from $REST_BOTTOM to $UP_BOTTOM px"
fi

# Away again. Back is how a keyboard is dismissed on Android, and the inset and
# the layout both have to come back to where they were: a host that only ever
# grows the inset leaves a hole at the bottom of every form for ever after.
"$ADB" -s "$SERIAL" shell input keyevent 4
sleep 3
DOWN="$(read_state || true)"
if [ -z "$DOWN" ]; then
  echo "  FAIL the tree could not be read with the keyboard away"
  exit 1
fi
DOWN_INSET="${DOWN% *}"
DOWN_BOTTOM="${DOWN#* }"
[ "$DOWN_INSET" = "$REST_INSET" ] && [ "$DOWN_BOTTOM" = "$REST_BOTTOM" ] && r=0 || r=1
check $r "dismissing it puts the inset and the field back where they were"
if [ "$r" -ne 0 ]; then
  echo "       inset $DOWN_INSET (was $REST_INSET), field bottom $DOWN_BOTTOM (was $REST_BOTTOM)"
fi

exit "$fail"
