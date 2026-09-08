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

# The manifest is read out of the APK, which is what the system reads. Looking
# at the input XML would not do: what gets installed is what came out of the
# link, and it is at the link that the file is chosen.
APK="$(build wearos)"
if [ ! -f "$APK" ]; then
  echo "  FAIL the watch APK never got built"
  exit 1
fi
echo "  ok   an wearos builds examples/hello-wear"

# Two readers, because the two assertions below are the whole point of this
# script and a missing aapt2 used to take both of them with it — a green run
# that had proved only that a file exists.
#
# `aapt2 dump badging` is the reading when aapt2 is installed. Without it the
# binary `AndroidManifest.xml` is pulled out of the APK and its NULs dropped:
# it is UTF-16 over an ASCII alphabet, so the string pool comes out readable,
# and aapt pools only the strings the document uses. Coarser — it cannot tell a
# `uses-feature` from any other element that names the same string — and an
# answer rather than a skip.
SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
AAPT2="$(ls -d "$SDK"/build-tools/*/aapt2 2>/dev/null | sort | tail -1)"

manifest() { # <apk>
  if [ -n "$AAPT2" ]; then
    "$AAPT2" dump badging "$1" || true
  else
    unzip -p "$1" AndroidManifest.xml 2>/dev/null | LC_ALL=C tr -d '\000' || true
  fi
}

if [ -n "$AAPT2" ]; then
  READING="aapt2 dump badging"
  WATCH_FEATURE="uses-feature: name='android.hardware.type.watch'"
  PACKAGE="package: name='dev.angularnative'"
else
  READING="the binary manifest, without aapt2"
  WATCH_FEATURE="android.hardware.type.watch"
  PACKAGE="dev.angularnative"
fi

DUMP="$(manifest "$APK")"
if [ -z "$DUMP" ]; then
  echo "  --   the watch APK's manifest could not be read: no aapt2 and no unzip"
  exit 0
fi
for expected in "$WATCH_FEATURE" "$PACKAGE"; do
  if ! grep -qF "$expected" <<<"$DUMP"; then
    echo "  FAIL the watch APK does not declare: $expected"
    exit 1
  fi
done
echo "  ok   the APK declares android.hardware.type.watch ($READING)"

# And the phone's does not declare it: if it did, the Play Store would stop
# offering it for phones and nobody would find out until it was published.
PHONE_APK="$(build android examples/hello-angular)"
if [ ! -f "$PHONE_APK" ]; then
  echo "  FAIL the phone APK never got built"
  exit 1
fi
if grep -qF "android.hardware.type.watch" <<<"$(manifest "$PHONE_APK")"; then
  echo "  FAIL the phone APK claims to be a watch one"
  exit 1
fi
echo "  ok   the phone's still does not declare it"
