#!/usr/bin/env bash
# Wear OS: that the watch APK really is the watch's, and that what does not
# belong says so.
#
# A Wear OS watch runs `android.view.View` like any phone, so almost everything
# about the watch that has to be checked is already checked by the other
# scripts: it is the same core, the same Java and the same primitives. What
# nobody checks is the little that does change, and that on top of that changes
# in different places that can drift apart from one another:
#
#   - the watch manifest, which is what declares the device's form factor;
#   - the theme, which now lives in a resource and not in the manifest;
#   - the list of primitives that are not mounted, which is in Java and
#     documented in `docs-site/src/content/docs/platforms/wearos.md`: if one enters the list and not the
#     document, the gap only shows up when somebody steps in it;
#   - that the APK `an wearos` produces is not the phone's, which is exactly the
#     failure nobody sees — it installs the same and starts the same.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== Wear OS"

python3 "$ROOT/scripts/check-wearos.py" "$ROOT"

LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT

# Build an APK and return its path. With `set -e` and `pipefail`, putting the
# build inside a `$(...)` and throwing its output at /dev/null makes a
# compilation failure kill the script without printing anything: the checker
# itself would go silent, which is the opposite of what is needed.
build() {
  if ! cargo an "$@" --no-launch >"$LOG" 2>&1; then
    echo "  FAIL 'cargo an $*' did not compile"
    tail -30 "$LOG"
    exit 1
  fi
  tail -1 "$LOG"
}

# The manifest is read out of the APK with `aapt2 dump`, which is what the
# system reads. Looking at the input XML would not do: what gets installed is
# what came out of the link, and it is at the link that the file is chosen.
APK="$(build wearos)"
if [ ! -f "$APK" ]; then
  echo "  FAIL the watch APK never got built"
  exit 1
fi
echo "  ok   an wearos builds examples/hello-wear"

SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
AAPT2="$(ls -d "$SDK"/build-tools/*/aapt2 2>/dev/null | sort | tail -1)"
if [ -z "$AAPT2" ]; then
  echo "  warn aapt2 not found; the APK's manifest cannot be read"
  exit 0
fi

DUMP="$("$AAPT2" dump badging "$APK")"
for expected in \
  "uses-feature: name='android.hardware.type.watch'" \
  "package: name='dev.angularnative'"
do
  if ! grep -qF "$expected" <<<"$(printf '%s' "$DUMP" || true)"; then
    echo "  FAIL the watch APK does not declare: $expected"
    exit 1
  fi
done
echo "  ok   the APK declares android.hardware.type.watch"

# And the phone's does not declare it: if it did, the Play Store would stop
# offering it for phones and nobody would find out until it was published.
PHONE_APK="$(build android examples/hello-angular)"
if [ ! -f "$PHONE_APK" ]; then
  echo "  FAIL the phone APK never got built"
  exit 1
fi
if grep -qF "name='android.hardware.type.watch'" <<<"$("$AAPT2" dump badging "$PHONE_APK" || true)"; then
  echo "  FAIL the phone APK claims to be a watch one"
  exit 1
fi
echo "  ok   the phone's still does not declare it"
