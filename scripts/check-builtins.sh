#!/usr/bin/env bash
# The built-in modules: that every host has all four, and that the platforms
# which cannot have one say so by name.
#
# `device` was the only module the framework brought, and `check-modules.sh`
# exists because two hosts of the seven had quietly stopped registering it. These
# four are four times that surface, and they have a second way of going wrong
# that `device` did not: a platform without the hardware. A television has
# nothing to vibrate and a watch has no share sheet, and the temptation in both
# cases is to answer something harmless — `false`, `null`, a promise that
# resolves and does nothing. That is the failure this file is mostly about.
#
# Six things, and they are six because each can break without the others
# noticing:
#
#   1. The list is one list. `BUILTIN_MODULES` in Rust, the Swift registry, the
#      Java registry and the TypeScript services are four copies of the same
#      four names, and nothing but this compares them.
#   2. Every host registers them and every host pumps them. A host that
#      registers and does not pump has an app whose promises never settle.
#   3. The C surface is declared the same in the three Apple headers, which are
#      never compiled together and so can drift apart in silence.
#   4. Every absence is stated by name. Not "unavailable": the platform, the
#      class that does not exist, and what to do instead.
#   5. The TypeScript says the same thing the shells do. Each module's platform
#      type excludes exactly the platforms whose shells refuse it.
#   6. And the whole thing runs. `examples/modules` calls all four and prints a
#      line per answer; macOS is the one platform where that can be watched
#      without a simulator, so the lines are read out of the running app.
#
# What needs a device is `check-builtins-device.sh`, which is out of
# `check-all.sh` for the same reason `check-a11y-device.sh` is.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== built-in modules"

fail=0
ok() { echo "  ok   $1"; }
ko() { echo "  FAIL $1"; fail=1; }
check() { if [ "$1" -eq 0 ]; then ok "$2"; else ko "$2"; fi; }

RUST="crates/an-bridge/src/builtins.rs"
SWIFT_REGISTRY="shells/shared/AnBuiltinModules.swift"
JAVA_REGISTRY="shells/android/java/dev/angularnative/AnBuiltinModules.java"
TS_DIR="packages/platform-native/src/modules"

# ── 1. One list, four copies ────────────────────────────────────────────────
NAMES="$(sed -n 's/^pub const BUILTIN_MODULES.*= &\[\(.*\)\];$/\1/p' "$RUST" \
  | tr -d '" ' | tr ',' '\n' | grep . | sort)"
if [ -z "$NAMES" ]; then
  ko "BUILTIN_MODULES could not be read from $RUST"
  exit 1
fi
COUNT="$(printf '%s\n' "$NAMES" | wc -l | tr -d ' ')"
ok "$COUNT built-in modules declared in Rust: $(printf '%s ' $NAMES)"

SWIFT_NAMES="$(grep -oE 'register\("[a-z]+"' "$SWIFT_REGISTRY" | cut -d'"' -f2 | sort)"
check "$([ "$NAMES" = "$SWIFT_NAMES" ] && echo 0 || echo 1)" \
  "the Swift registry registers exactly those names"
JAVA_NAMES="$(grep -oE 'register\("[a-z]+"' "$JAVA_REGISTRY" | cut -d'"' -f2 | sort)"
check "$([ "$NAMES" = "$JAVA_NAMES" ] && echo 0 || echo 1)" \
  "the Java registry registers exactly those names"

# And that TypeScript offers a service per name. A module nobody can call from
# an app is a module that does not exist.
for module in $NAMES; do
  if [ -f "$TS_DIR/$module.ts" ] &&
     grep -q "'$module'" "$TS_DIR/$module.ts" &&
     grep -q "from './modules/$module'" packages/platform-native/src/public-api.ts; then
    ok "the $module module has a typed service and is exported"
  else
    ko "$module is in BUILTIN_MODULES with no exported service in $TS_DIR"
  fi
done

# ── 2. Every host registers them, and every host pumps them ─────────────────
for host in an-ios an-macos an-watch; do
  grep -q 'builtins::builtin_modules()' "crates/$host/src/ffi.rs" && r=0 || r=1
  check $r "$host registers the built-in modules"
  grep -q 'builtins::pump_c()' "crates/$host/src/ffi.rs" && r=0 || r=1
  check $r "and serves them once a frame, on the main thread"
done
grep -q 'builtins::builtin_modules()' crates/an-android/src/jni_bridge.rs && r=0 || r=1
check $r "an-android registers the built-in modules"
grep -q 'builtins::pump(env)' crates/an-android/src/jni_bridge.rs && r=0 || r=1
check $r "and serves them once a frame, on the UI thread"

# ── 3. The C surface, declared three times and never compiled together ──────
SYMBOLS="an_builtin_set_dispatch an_builtin_resolve an_builtin_reject"
for header in crates/an-ios/include/angular_native.h \
              crates/an-macos/include/angular_native_macos.h \
              crates/an-watch/include/angular_native_watch.h; do
  missing=""
  for symbol in $SYMBOLS; do
    grep -q "$symbol" "$header" || missing="$missing $symbol"
  done
  check "$([ -z "$missing" ] && echo 0 || echo 1)" \
    "$(basename "$header") declares the built-in bridge$missing"
done

# ── 4. Every absence is stated by name ──────────────────────────────────────
#
# The table is "file : platform : what the sentence has to name". Take a row out
# and the check stops covering it; add a platform to a module and the row is
# where it is written down.
absent() { # <file> <what has to appear, one per argument>
  local file="$1"; shift
  local what missing=""
  for what in "$@"; do
    grep -qF -- "$what" "$file" || missing="$missing [$what]"
  done
  check "$([ -z "$missing" ] && echo 0 || echo 1)" \
    "$(basename "$file") names what it cannot do$missing"
}

absent shells/shared/AnBuiltinFiles.swift \
  "tvOS gives an app no storage that lasts" \
  "tvOS has no file picker" \
  "watchOS has no file picker"
absent shells/shared/AnBuiltinShare.swift \
  "tvOS has no share sheet" \
  "watchOS has no share sheet"
absent shells/shared/AnBuiltinHaptics.swift \
  "tvOS has no haptics" \
  "visionOS has no haptics" \
  "macOS has no notification haptics"
absent shells/android/java/dev/angularnative/AnFiles.java \
  "Wear OS has no file picker"
absent shells/android/java/dev/angularnative/AnShare.java \
  "no app on this watch can receive a share"

# ── 5. The TypeScript says the same thing ───────────────────────────────────
#
# Each module's platform type excludes exactly the platforms whose shell refuses
# it. It is the one place an app reads, so a type that is more generous than the
# shells is a lie an app compiles against.
excludes() { # <file> <exported type> <the platforms, sorted, space separated>
  local declared
  declared="$(sed -n "s/^export type $2 = Exclude<NativePlatform, \(.*\)>$/\1/p" "$1" \
    | tr -d "' " | tr '|' '\n' | sort | tr '\n' ' ' | sed 's/ $//')"
  if [ "$declared" = "$3" ]; then
    ok "$2 excludes exactly $3"
  else
    ko "$2 excludes '$declared' and the shells refuse '$3'"
  fi
}

excludes "$TS_DIR/files.ts" FilePickerPlatform "tvos watchos wearos"
excludes "$TS_DIR/files.ts" DurableStoragePlatform "tvos"
excludes "$TS_DIR/share.ts" SharePlatform "tvos watchos"
excludes "$TS_DIR/haptics.ts" HapticsPlatform "tvos visionos"
# The one with nothing to exclude says so as itself, not as an empty `Exclude`.
grep -q '^export type NetworkPlatform = NativePlatform$' "$TS_DIR/network.ts" && r=0 || r=1
check $r "NetworkPlatform is every platform, which is the honest way to write no exclusions"

# ── 6. Android's permissions: declared, and checked before use ──────────────
#
# Both are install-time, so there is no dialog and nothing the person can grant
# later: an app whose manifest lacks one has a module that can only ever fail.
# The framework declares them, and each module checks rather than letting the
# call throw.
for manifest in shells/android/AndroidManifest.xml shells/android/AndroidManifest.wear.xml; do
  missing=""
  for permission in ACCESS_NETWORK_STATE VIBRATE; do
    grep -q "android.permission.$permission" "$manifest" || missing="$missing $permission"
  done
  check "$([ -z "$missing" ] && echo 0 || echo 1)" \
    "$(basename "$manifest") declares what the built-ins need$missing"
done
grep -q 'checkSelfPermission' shells/android/java/dev/angularnative/AnNetwork.java && r=0 || r=1
check $r "the network module checks for its permission instead of letting the call throw"
grep -q 'checkSelfPermission' shells/android/java/dev/angularnative/AnHaptics.java && r=0 || r=1
check $r "and so does the haptics one"
# And that the refusal is an instruction and not a verdict: the line to paste,
# in both of them, so that neither drifts into merely saying no.
for module in AnNetwork AnHaptics; do
  grep -q 'uses-permission' "shells/android/java/dev/angularnative/$module.java" && r=0 || r=1
  check $r "and $module's rejection carries the manifest line to paste"
done

# A file cannot leave an Android app as a path, so the shell brings a
# FileProvider. Without it `share` with a file throws where the app cannot see
# it, which is the case the instruction in `AnShare` is written for.
grep -q 'androidx.core.content.FileProvider' shells/android/AndroidManifest.xml && r=0 || r=1
check $r "the shell declares the FileProvider a shared file leaves through"
[ -f shells/android/res/xml/an_file_paths.xml ] && r=0 || r=1
check $r "and the paths it is allowed to hand over"

# Package visibility: without `<queries>` an Android 11 device answers `null` to
# every `resolveActivity`, and both modules would report that a device with a
# perfectly good picker has none.
for manifest in shells/android/AndroidManifest.xml shells/android/AndroidManifest.wear.xml; do
  grep -q '<queries>' "$manifest" && r=0 || r=1
  check $r "$(basename "$manifest") declares what the modules look for"
done

if [ "$(uname -s)" != "Darwin" ]; then
  echo "  --   the rest is skipped: the macOS host only builds on a Mac"
  exit "$fail"
fi

# ── 7. And the whole thing runs ─────────────────────────────────────────────
#
# `examples/modules` calls all four and prints `<module>: ok …` or
# `<module>: no …` per answer. Both are read: what has to be `ok` on a desktop,
# and what has to be there at all. Before any of this existed the four names
# rejected with "there is no native module called …", which is what a registry
# nobody wired up looks like from an app.
BUILD_LOG="$(mktemp)"
RUN_LOG="$(mktemp)"
trap 'rm -f "$BUILD_LOG" "$RUN_LOG"' EXIT

if ! cargo an macos examples/modules --no-launch >"$BUILD_LOG" 2>&1; then
  ko "the modules example does not build for macOS"
  tail -30 "$BUILD_LOG"
  exit 1
fi
ok "an macos builds examples/modules"

BIN="$ROOT/build/macos/AngularNativeMac.app/Contents/MacOS/AngularNativeMac"
AN_SCREENSHOT="$ROOT/build/macos/builtins.png" AN_SCREENSHOT_FRAMES=150 AN_DUMP_TEXT=1 \
  "$BIN" >"$RUN_LOG" 2>&1 || true

# What a Mac has to be able to do. The exact prefix, because `files: ok` and
# `files: no` differ by two characters and the whole point is which one it is.
for line in \
  'files.documentsDirectory: ok' \
  'files: ok wrote and read back' \
  'files.sandbox: ok' \
  'share.canShare: ok yes' \
  'network.status: ok online' \
  'haptics.support: ok yes'
do
  if grep -qF "[modules] $line" "$RUN_LOG"; then
    ok "on macOS: $(grep -oF -m1 "$line" "$RUN_LOG")"
  else
    ko "on macOS the app never printed '$line'"
    grep -F '[modules]' "$RUN_LOG" | sed 's/^/       /' | head -10
  fi
done

# And the one thing a Mac says it cannot do, said in the caveat rather than
# hidden behind a boolean.
if grep -q 'Force Touch trackpad' "$RUN_LOG"; then
  ok "and it says the haptics may reach nothing on a Mac without a Force Touch trackpad"
else
  ko "macOS answered haptics support without the caveat about the trackpad"
fi

# Not one of the four may come back as a name nobody registered. It is the
# failure `check-modules.sh` was written for, one module later.
if grep -q 'there is no native module called' "$RUN_LOG"; then
  ko "a built-in module is not registered: $(grep -o 'there is no native module called.*' "$RUN_LOG" | head -1)"
fi

if [ "$fail" -ne 0 ]; then
  echo
  grep -F '[modules]' "$RUN_LOG" | tail -20
  exit 1
fi
