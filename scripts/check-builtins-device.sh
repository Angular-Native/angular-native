#!/usr/bin/env bash
# The built-in modules on a real Android, phone or watch.
#
# It is out of `check-all.sh` on purpose, like `check-a11y-device.sh`: it builds
# an APK, installs it and reads what the app says about the hardware it is
# actually on. Half of what these modules do cannot be checked any other way —
# whether there is a vibrator, whether anything on this device answers
# ACTION_SEND, what the network really is — and a Mac cannot answer any of it.
#
# It also covers the one platform of the seven that the desktop check cannot say
# anything about: Wear OS. Run it on the watch emulator to see the two refusals
# that only exist there.
#
#   scripts/check-builtins-device.sh                 # the one device that is ready
#   scripts/check-builtins-device.sh emulator-5554   # or the one you name
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== built-in modules on a device"

fail=0
ok() { echo "  ok   $1"; }
ko() { echo "  FAIL $1"; fail=1; }

SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
ADB="$SDK/platform-tools/adb"
if [ ! -x "$ADB" ]; then
  ko "no adb in $SDK/platform-tools"
  exit 1
fi

# Which device. A phone plugged in and an emulator running at the same time is
# the normal state of this machine, and an `adb` with no `-s` refuses to choose.
SERIAL="${1:-}"
if [ -z "$SERIAL" ]; then
  READY="$("$ADB" devices | awk '$2 == "device" { print $1 }')"
  COUNT="$(printf '%s\n' "$READY" | grep -c . || true)"
  if [ "$COUNT" -eq 0 ]; then
    ko "no device ready: start an emulator or plug a phone in"
    "$ADB" devices
    exit 1
  fi
  if [ "$COUNT" -gt 1 ]; then
    ko "more than one device ready; name the one to use:"
    printf '%s\n' "$READY" | sed 's/^/         /'
    exit 1
  fi
  SERIAL="$READY"
fi

# A watch is a different set of expectations, not a smaller one: `share` and
# `files.pick` have to *fail* there, and by name. Which of the two this is
# decides what is looked for below.
if "$ADB" -s "$SERIAL" shell getprop ro.build.characteristics | grep -q watch; then
  FORM="watch"
else
  FORM="phone"
fi
ok "device $SERIAL is a $FORM"

LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT

# `an wearos` is not a flag on `an android`: it is a different manifest, a
# different theme and a different device. Which one is built follows from what
# is plugged in.
COMMAND="android"
[ "$FORM" = watch ] && COMMAND="wearos"
if ! cargo an "$COMMAND" examples/modules --no-launch >"$LOG" 2>&1; then
  ko "the APK did not build"
  tail -30 "$LOG"
  exit 1
fi
APK="$(tail -1 "$LOG")"
if [ ! -f "$APK" ]; then
  ko "the APK did not build"
  tail -30 "$LOG"
  exit 1
fi
ok "an android builds examples/modules"

if ! "$ADB" -s "$SERIAL" install -r "$APK" >"$LOG" 2>&1; then
  ko "the APK did not install"
  tail -10 "$LOG"
  exit 1
fi
"$ADB" -s "$SERIAL" shell am force-stop dev.angularnative

# Where this run starts in the log.
#
# `logcat -c` is not used: on some emulators it fails outright —"failed to clear
# the 'main,system' logs"— and on others it clears one buffer and not the other,
# and a check that dies there dies for a reason that has nothing to do with what
# it checks. A timestamp works everywhere and cannot lose somebody else's lines.
SINCE="$("$ADB" -s "$SERIAL" shell "date +'%m-%d %H:%M:%S.000'" | tr -d '\r')"
"$ADB" -s "$SERIAL" shell am start -n dev.angularnative/.MainActivity >/dev/null

# What is waited for is the thing the check needs and not a fixed number of
# seconds: the last line the app prints on its own. An emulator can take fifteen
# seconds to get there and a phone two.
FOUND=""
for _ in $(seq 1 60); do
  if "$ADB" -s "$SERIAL" logcat -d -t "$SINCE" 2>/dev/null | grep -q '\[modules\] haptics.support:'; then
    FOUND="yes"
    break
  fi
  sleep 1
done
if [ -z "$FOUND" ]; then
  ko "the app never got as far as haptics.support after 60 s"
  "$ADB" -s "$SERIAL" logcat -d -t "$SINCE" | grep -iE 'angular-native|AndroidRuntime' | tail -30
  exit 1
fi
"$ADB" -s "$SERIAL" logcat -d -t "$SINCE" | grep -F '[modules]' > "$LOG"
ok "the app answered on the device"

says() { # <what has to be in the log> <what it means>
  if grep -qF -- "$1" "$LOG"; then
    ok "$2"
  else
    ko "$2 — the app never said '$1'"
  fi
}

# The app's own container: the same on both form factors, and the half of the
# module that needs nobody's permission.
says 'files.documentsDirectory: ok' 'files answers the app its own directory'
says 'files: ok wrote and read back' 'and writes, reads, lists and deletes inside it'
says 'files.sandbox: ok' 'and refuses a path outside it'

# The network monitor exists everywhere, watch included.
says 'network.status: ok' 'the network monitor answers'

# And the two that differ. On a phone there is a chooser and a vibrator; on a
# watch there is a vibrator and nothing that receives a share.
if [ "$FORM" = watch ]; then
  # Either answer is the truth on a watch and neither can be assumed: a Wear
  # watch resolves ACTION_SEND to Bluetooth and to whatever the manufacturer
  # added, so one has somewhere to share to and the next has nowhere. What is
  # checked is that the module asked the watch instead of deciding from the
  # platform — which is what the `an-wear` emulator caught: it answers yes.
  says 'share.canShare: ok' 'and the watch answers from what is installed on it'
  echo "       ($(sed -n 's/.*\(share.canShare: .*\)$/\1/p' "$LOG" | head -1))"
else
  says 'share.canShare: ok yes' 'and the phone has somewhere to share to'
fi
says 'haptics.support: ok' 'and the haptics module says what this device can do'

# Nothing may come back as a name nobody registered: that is a shell that did
# not install its modules, and it looks exactly like a broken app from inside.
if grep -q 'there is no native module called' "$LOG"; then
  ko "a built-in module is not registered on this device"
fi

if [ "$fail" -ne 0 ]; then
  echo
  cat "$LOG"
  exit 1
fi
