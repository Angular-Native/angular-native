#!/usr/bin/env bash
# Biometrics and keychain: that the methods answer and that what they answer
# shows.
#
# The headless runner has neither Swift nor Java, so both plugins are mounted
# with `AN_PLUGINS`, which gives a canned answer per method. What is checked is
# not the biometrics —that has to be seen on a device— but the whole path: the
# app calls, the promise resolves, and what the plugin answered ends up on screen
# with the sentence that goes with it.
#
# The half that matters most is the one below: **with no plugin the promise is
# rejected and it shows**. A method that swallowed the call would leave this same
# screen with a dash on it and no hint at all.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0
ok() { echo "  ok   $1"; }
ko() { echo "  FAIL $1"; fail=1; }

contains() {
  if grep -qF -e "$2" <<<"$1"; then ok "$3"; else ko "$3"; fi
}

echo "== biometrics and keychain"

cargo an build examples/secrets >/dev/null
ok 'the bundle compiles with both plugins resolved'

BUNDLE=build/bundle/secrets/main.js
headless() { cargo run -q -p an-bridge --example headless -- "$BUNDLE" "$1" 2>&1; }

# ── Everything answers ──────────────────────────────────────────────────────
#
# The simulated tap lands on the first node that listens, which is "save".
ALL='{
  "biometrics": {
    "availability": { "status": "available", "kind": "faceId", "detail": "fake" },
    "authenticate": { "outcome": "success", "kind": "faceId", "detail": "fake" }
  },
  "keychain": {
    "has": true,
    "set": { "outcome": "saved", "detail": "fake" },
    "get": { "outcome": "found", "value": "test secret", "detail": "fake" },
    "remove": true
  }
}'
# The sensor line and the one saying whether anything is stored are signals
# separate from the last result, so the simulated tap does not run them over.
STARTUP="$(AN_PLUGINS="$ALL" headless 6)"
contains "$STARTUP" 'fake plugin: biometrics' \
  'the biometrics module is registered under its name'
contains "$STARTUP" 'fake plugin: keychain' \
  'and the keychain one too'
contains "$STARTUP" 'Face ID, ready' 'availability arrives and the app says which sensor there is'
contains "$STARTUP" 'there is a secret stored' 'keychain.has arrives without asking for biometrics'

# And six, which is enough for the simulated tap. It lands on the first node that
# listens, which is "save", so what shows afterwards is what `set` answered.
WITH="$(AN_PLUGINS="$ALL" headless 6)"
contains "$WITH" 'your face is now needed' 'and the tap stored it: set resolved'

# ── Every biometric ending has its own sentence ─────────────────────────────
#
# It is the reason `authenticate` does not return a boolean. Three of the eleven
# are checked, which are the three that ask the app to do different things.
not_recognised() { # $1 outcome  $2 piece of the sentence that has to come out
  local answer
  answer="$(AN_PLUGINS='{
    "biometrics": { "availability": { "status": "'"$1"'", "kind": "none", "detail": "fake" } },
    "keychain": { "has": false }
  }' headless 4)"
  contains "$answer" "$2" "availability=$1 comes out as \"$2\""
}
#
# The pieces are short because the dump truncates node text at forty characters
# and the sensor name already eats eleven of them.
not_recognised noHardware 'this device has no biometric'
not_recognised notEnrolled 'there is no face or finger'
not_recognised lockedOut 'too many attempts'

# ── With no plugin, the promise is rejected and it shows ────────────────────
#
# The runner's dump truncates text at forty characters, so what is compared is
# the opening of the rejection. Those first words come out of a crate, so both
# languages are accepted.
WITHOUT="$(headless 4)"
contains "$WITHOUT" 'failed: Error: there is no native module' \
  'with no plugin the promise is rejected saying the module does not exist'

# ── A method with no canned answer is not swallowed either ──────────────────
#
# The keychain answers `has` but not `set`, and the tap calls `set`.
HALFWAY="$(AN_PLUGINS='{
  "biometrics": { "availability": { "status": "available", "kind": "faceId", "detail": "x" } },
  "keychain": { "has": true }
}' headless 6)"
# The method name goes inside the message, and the plugin itself answers for
# that.
contains "$HALFWAY" 'failed: Error: the keychain plugin' \
  'a method the plugin does not serve rejects the promise'

# ── And that the Android side compiles ──────────────────────────────────────
#
# The headless runner touches neither Swift nor Java. Building the APK does, and
# it costs half a minute —`check-android-java.sh` does that—, but a plugin's Java
# can be checked on its own and in a second: against `android.jar` and with the
# shell directory on the `sourcepath`, which is what resolves `AnPlugin` and
# `AnPluginCall` without dragging in the whole host.
#
# `-implicit:none` is what cuts that dragging: the sources that are named are
# compiled and nothing else. An error inside the shell does not come out here
# —that is what the APK is for—; an error in the plugin does. This check is the
# one that found that `BIOMETRIC_ERROR_NEGATIVE_BUTTON` does not exist in the
# platform API.
ANDROID_JAR="$(ls -d "${ANDROID_HOME:-$HOME/Library/Android/sdk}"/platforms/*/android.jar 2>/dev/null | tail -1)"
if [ -z "$ANDROID_JAR" ]; then
  ko 'android.jar not found; the plugins Java goes unchecked'
else
  CLASSES="$(mktemp -d)"
  trap 'rm -rf "$CLASSES"' EXIT
  # `find` and not a glob: bash does not expand `**` without `globstar`, and
  # without it the wildcard would reach `javac` as it stands and the check would
  # fail for a reason that has nothing to do with the code.
  find packages -path '*/native/android/*' -name '*.java' >"$CLASSES/sources.txt"
  if javac -nowarn -implicit:none -source 17 -target 17 \
      -classpath "$ANDROID_JAR" \
      -sourcepath shells/android/java \
      -d "$CLASSES" \
      @"$CLASSES/sources.txt" >"$CLASSES/javac.log" 2>&1; then
    ok 'javac compiles the Android side of the plugins against android.jar'
  else
    ko 'the Android side of some plugin does not compile'
    tail -20 "$CLASSES/javac.log"
  fi
fi

exit "$fail"
