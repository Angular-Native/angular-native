#!/usr/bin/env bash
# A plugin's files: that they reach the bundle on every platform, that the code
# can read them there, and that two plugins cannot quietly overwrite each other.
#
# A plugin used to be able to contribute methods and one kind of view, and
# nothing else. Anything it had to read by name —a mark, a `.strings`, a sound—
# it either drew in code or asked the app to carry for it, and the second is not
# a plugin: it is a plugin plus an installation instruction nobody follows.
#
# Three things are checked here and they are not the same claim:
#
#   1. **The refusals**, from the manifests alone. Two plugins shipping one file
#      name, a plugin landing on the app's own name, a plugin shipping `main.js`.
#      All of them come out of `an plugins --platform`, in a second, before
#      anything is compiled.
#   2. **The file is in the artefact.** The `.app` root on iOS, `assets/` inside
#      the APK, `Contents/Resources` on the Mac. A file on disk is not a file in
#      the archive: the APK is a zip assembled by hand, and a resource copied
#      into the staging directory and left out of the zip is exactly the kind of
#      thing that passes an `ls`.
#   3. **The code can read it.** That is the one that needs a running app, and
#      the Mac is the only platform where the check can have one. `an macos`
#      builds the clipboard example, the app screenshots itself, and what is
#      counted is the plugin's own two colours on screen. `imageNamed:` returning
#      nil leaves an empty view and says nothing.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"
AN="$TARGET/debug/an"
WORK="$ROOT/build/check-plugin-resources"

fail=0
ok() { echo "  ok   $1"; }
ko() { echo "  FAIL $1"; fail=1; }
contains() {
  if grep -qE -e "$2" <<<"$1"; then ok "$3"; else ko "$3"; echo "$1" | sed 's/^/       /' | head -8; fi
}
# Runs something that has to fail, and hands back what it said.
must_fail() {
  local output
  if output="$("$@" 2>&1)"; then
    ko "expected a failure and it succeeded: $*"
    echo "$output" | sed 's/^/       /' | head -8
    return 0
  fi
  printf '%s' "$output"
}

echo "== plugin resources"

cargo build -q -p an-cli

# ── 1. The refusals ─────────────────────────────────────────────────────────
#
# With fixture packages under `build/`, so that no sham plugin goes into the
# repo's workspaces. `node_modules` inside the app is where Node —and `an`—
# look first.
plugin() { # $1 app dir  $2 package  $3 resource path inside the plugin
  local dir="$1/node_modules/$2"
  mkdir -p "$dir/res/$(dirname "$3")"
  cat >"$dir/package.json" <<JSON
{
  "name": "$2",
  "version": "0.0.1",
  "angularNative": {
    "module": "$(echo "$2" | tr -cd '[:alnum:]')",
    "resources": "res",
    "ios": { "sources": "native", "register": "Whatever" }
  }
}
JSON
  mkdir -p "$dir/native"
  echo '// only so the directory has a source in it' >"$dir/native/Whatever.swift"
  echo 'not code' >"$dir/res/$3"
}

app() { # $1 dir  $2… packages
  rm -rf "$1"
  mkdir -p "$1"
  local names=""
  shift 1
  for package in "$@"; do names="$names\"$package\": \"0.0.1\","; done
  cat >"$WORK/app/package.json" <<JSON
{ "name": "@fixture/resources", "private": true,
  "dependencies": { ${names%,} } }
JSON
  echo '{}' >"$WORK/app/tsconfig.json"
}

rm -rf "$WORK"
mkdir -p "$WORK"
app "$WORK/app" "@fixture/one" "@fixture/two"
plugin "$WORK/app" "@fixture/one" "icon.png"
plugin "$WORK/app" "@fixture/two" "icon.png"

output="$(must_fail "$AN" plugins build/check-plugin-resources/app --platform ios)"
contains "$output" '@fixture/one' 'two plugins shipping one file name name the first of them'
contains "$output" '@fixture/two' 'and the second: nobody reading this wrote either package'
contains "$output" 'icon\.png' 'and the name they are fighting over'
contains "$output" 'with nothing said' 'and says what the silent version of it would do'

# The app's own file against a plugin's. The app is the thing being built, so it
# is named as the app rather than as a package, and it is still an error.
app "$WORK/app" "@fixture/one"
plugin "$WORK/app" "@fixture/one" "icon.png"
mkdir -p "$WORK/app/resources"
echo 'the app drew this one' >"$WORK/app/resources/icon.png"
output="$(must_fail "$AN" plugins build/check-plugin-resources/app --platform ios)"
contains "$output" "the app's own" 'a plugin landing on a name the app itself uses is refused'
contains "$output" '@fixture/one' 'naming the plugin that would have replaced it'
rm -rf "$WORK/app/resources"

# And the name the build writes itself. This one is worse than a collision
# between two pictures: `main.js` is the app.
app "$WORK/app" "@fixture/one"
plugin "$WORK/app" "@fixture/one" "main.js"
output="$(must_fail "$AN" plugins build/check-plugin-resources/app --platform ios)"
contains "$output" 'the name the build writes' 'a plugin shipping main.js is refused before any build'
contains "$output" '@fixture/one' 'saying whose rename it is to make'

# Nothing colliding: it passes, and that is the half that keeps the three above
# from being a check that always fails.
app "$WORK/app" "@fixture/one" "@fixture/two"
plugin "$WORK/app" "@fixture/one" "one.png"
plugin "$WORK/app" "@fixture/two" "nested/two.png"
if "$AN" plugins build/check-plugin-resources/app --platform ios >/dev/null 2>&1; then
  ok 'two plugins with different file names pass, subdirectories included'
else
  ko 'two plugins with different file names should pass'
  "$AN" plugins build/check-plugin-resources/app --platform ios 2>&1 | sed 's/^/       /' | head -8
fi
rm -rf "$WORK/app"

# ── 2. The real plugin, listed and then built ───────────────────────────────
#
# `@angular-native/plugin-clipboard` ships one PNG, and `examples/clipboard`
# draws it without owning it. Everything below is about that one file.
LIST="$("$AN" plugins examples/clipboard 2>&1)"
contains "$LIST" 'resources: packages/plugin-clipboard/resources' \
  'an plugins says which directory a plugin ships files from'

MARK=clipboard-mark.png

if [ "$(uname -s)" != "Darwin" ]; then
  echo "  --   the Apple halves are skipped: they only build on a Mac"
else
  LOG="$(mktemp)"
  if cargo an ios examples/clipboard --no-launch >"$LOG" 2>&1; then
    if [ -f "$ROOT/build/ios/AngularNative.app/$MARK" ]; then
      ok "the plugin's file is at the root of the iOS .app, where UIImage(named:) looks"
    else
      ko "the plugin's file never reached the iOS .app"
    fi
  else
    ko 'the clipboard example does not build for iOS'
    tail -20 "$LOG" | sed 's/^/       /'
  fi

  if cargo an macos examples/clipboard --no-launch >"$LOG" 2>&1; then
    contains "$(cat "$LOG")" "$MARK \(from @angular-native/plugin-clipboard\)" \
      'the build says which package each resource came from'
    if [ -f "$ROOT/build/macos/AngularNativeMac.app/Contents/Resources/$MARK" ]; then
      ok "and the file is in Contents/Resources, beside main.js"
    else
      ko "the plugin's file never reached the macOS .app"
    fi
  else
    ko 'the clipboard example does not build for macOS'
    tail -20 "$LOG" | sed 's/^/       /'
  fi
  rm -f "$LOG"
fi

# ── 3. The one that needs it to run ─────────────────────────────────────────
#
# Everything above proves the file is somewhere. This proves the host opened it,
# decoded it and drew it, which is a different claim: `NSImage(named:)` that
# finds nothing returns nil and leaves an empty view, and an empty view looks
# exactly like a view whose image has not loaded yet.
#
# What is counted is the mark's two colours in the corner where the example puts
# it. Both of them, on purpose: the amber alone would also be an `NSImageView`
# with a background colour and no image inside it, and the dark square is what
# says the PNG was decoded.
APP="$ROOT/build/macos/AngularNativeMac.app"
SHOT="$ROOT/build/macos/plugin-resources.png"
if [ "$(uname -s)" != "Darwin" ] || [ ! -x "$APP/Contents/MacOS/AngularNativeMac" ]; then
  echo "  --   the running app is skipped: there is no macOS .app to launch"
elif ! command -v ffmpeg >/dev/null 2>&1 || ! command -v python3 >/dev/null 2>&1; then
  echo "  --   the running app is skipped: reading the pixels needs ffmpeg and python3"
else
  RUN_LOG="$(mktemp)"
  rm -f "$SHOT"
  if AN_SCREENSHOT="$SHOT" "$APP/Contents/MacOS/AngularNativeMac" >"$RUN_LOG" 2>&1; then
    ok 'the app with the plugin starts, mounts the tree and closes on its own'
  else
    ko 'the app never mounted anything'
    tail -20 "$RUN_LOG" | sed 's/^/       /'
  fi
  PIXELS="$(python3 - "$SHOT" <<'PYEOF'
import subprocess, sys
raw = subprocess.run(
    ["ffmpeg", "-v", "error", "-i", sys.argv[1], "-vf", "crop=120:120:0:40",
     "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
    capture_output=True).stdout
amber = ink = 0
for i in range(0, len(raw), 3):
    pixel = tuple(raw[i:i + 3])
    if pixel == (255, 176, 0):
        amber += 1
    elif pixel == (17, 17, 17):
        ink += 1
print(amber, ink)
PYEOF
)"
  set -- $PIXELS
  if [ "${1:-0}" -gt 900 ] && [ "${2:-0}" -gt 150 ]; then
    ok "a file the app does not own is drawn on screen ($1 pixels of the plugin's amber, $2 of its mark)"
  else
    ko "the plugin's PNG never reached the screen: amber=${1:-none} ink=${2:-none}"
    echo "       the file is in the bundle; what failed is the host finding it by the name"
    echo "       the plugin's package.json gave it"
  fi
  rm -f "$RUN_LOG"
fi

# ── 4. Android, where the destination was a decision ────────────────────────
#
# `assets/` and not `res/`: a drawable under `res/` is reached through a
# generated `R` class in the app's package, and a plugin cannot name an id
# linked into a package it does not own. `getAssets().open` takes the name from
# the manifest as it is. What is checked is the entry inside the zip, because
# the APK is assembled by hand and a file in the staging directory that never
# became an entry is invisible to everything but `unzip`.
if [ ! -s "$ROOT/vendor/android/build/classpath.txt" ]; then
  echo "  --   Android is skipped: the prepared dependencies are not here"
  echo "       python3 scripts/fetch-android-deps.py && python3 scripts/prepare-android-deps.py"
else
  LOG="$(mktemp)"
  if cargo an android examples/clipboard --no-launch >"$LOG" 2>&1; then
    APK="$(tail -1 "$LOG")"
    if grep -qx "assets/$MARK" <<<"$(unzip -Z1 "$APK" 2>/dev/null || true)"; then
      ok "the plugin's file is an entry of the APK, under assets/"
    else
      ko "the plugin's file is not in the APK"
      unzip -Z1 "$APK" 2>/dev/null | grep '^assets/' | sed 's/^/       /'
    fi
  else
    ko 'the clipboard example does not build for Android'
    tail -20 "$LOG" | sed 's/^/       /'
  fi
  rm -f "$LOG"
fi

rm -rf "$WORK"
exit "$fail"
