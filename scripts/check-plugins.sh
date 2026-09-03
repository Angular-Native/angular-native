#!/usr/bin/env bash
# Plugins: that they are discovered, that they link and that they answer.
#
# It covers the three halves of the system with no simulator and no emulator:
#
#   1. Discovery — `an plugins` finds the plugin through the dependencies of the
#      app's package.json, and says which platforms it covers.
#   2. The missing platform owns up — a plugin that only ships iOS, for the
#      Android build, with a message that says which one and why.
#   3. The call goes out and comes back — the example calls the module, the
#      promise resolves and the answer ends up on screen; with no plugin, it is
#      rejected and it shows.
#
# And then the part that does take half a minute but really compiles: building
# the example's .app and APK. That is where it is checked that the plugin's
# Swift and Java sources go into the same `swiftc` and `javac` invocation as the
# shell, and that the generated registry compiles.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0
ok() { echo "  ok   $1"; }
ko() { echo "  FAIL $1"; fail=1; }

# Check that some output contains a pattern. The `-e` is not redundant: there
# are patterns starting with a dash, and without it grep takes them for options
# of its own.
contains() {
  if grep -qE -e "$2" <<<"$1"; then ok "$3"; else ko "$3"; fi
}

echo "== plugins"

# ── 1. Discovery ────────────────────────────────────────────────────────────
LIST="$(cargo an plugins examples/clipboard 2>&1)"
contains "$LIST" '^clipboard  \(@angular-native/plugin-clipboard\)' \
  'the plugin is discovered through the package.json dependency'
contains "$LIST" 'ios \+ android' 'and it says it covers both platforms'
contains "$LIST" 'packages/plugin-clipboard' 'and where the package came from'

# The CLI is a crate and is being translated on its own branch, so the patterns
# that read its prose accept either language.
NONE="$(cargo an plugins examples/kitchen 2>&1)"
contains "$NONE" '(no depende de ningún plugin|depends on no plugin|not depend on any plugin)' \
  'an app with no plugins says so and does not invent one'

for platform in ios android; do
  if cargo an plugins examples/clipboard --platform "$platform" >/dev/null 2>&1; then
    ok "the example passes the $platform check"
  else
    ko "the example should pass the $platform check"
  fi
done

# ── 2. The missing platform owns up ─────────────────────────────────────────
#
# With a fake plugin that only declares iOS. It is set up under `build/` so as
# not to put a sham package into the repo's workspaces: what is being tested is
# dependency resolution, and `node_modules` inside the app is exactly where Node
# —and `an`— look first.
FIXTURE="$ROOT/build/plugins-fixture"
rm -rf "$FIXTURE"
mkdir -p "$FIXTURE/app/node_modules/@fixture/ios-only/native/ios"
cat >"$FIXTURE/app/package.json" <<'JSON'
{
  "name": "@fixture/app-ios-only",
  "private": true,
  "dependencies": { "@fixture/ios-only": "0.0.1" }
}
JSON
# `an` only asks that it exists to recognise the directory as an app.
echo '{}' >"$FIXTURE/app/tsconfig.json"
cat >"$FIXTURE/app/node_modules/@fixture/ios-only/package.json" <<'JSON'
{
  "name": "@fixture/ios-only",
  "version": "0.0.1",
  "angularNative": {
    "module": "iosonly",
    "ios": { "sources": "native/ios", "register": "IosOnlyPlugin" }
  }
}
JSON
echo '// only so the directory has a source in it' \
  >"$FIXTURE/app/node_modules/@fixture/ios-only/native/ios/IosOnlyPlugin.swift"

if cargo an plugins build/plugins-fixture/app --platform ios >/dev/null 2>&1; then
  ok 'a plugin that only ships iOS passes the iOS check'
else
  ko 'a plugin that only ships iOS should pass the iOS check'
fi

if NEGATIVE="$(cargo an plugins build/plugins-fixture/app --platform android 2>&1)"; then
  ko 'building for Android with a plugin that only ships iOS should fail'
else
  ok 'building for Android with a plugin that only ships iOS fails'
  contains "$NEGATIVE" '@fixture/ios-only' 'and the message says which package it is'
  contains "$NEGATIVE" '(no se puede compilar para Android|cannot be built for Android)' \
    'and for which platform'
  contains "$NEGATIVE" 'angularNative.android' 'and what has to be done to fix it'
fi
rm -rf "$FIXTURE"

# ── 3. The call goes out and comes back ─────────────────────────────────────
cargo an build examples/clipboard >/dev/null
ok 'the bundle compiles with the plugin import resolved'

# With the plugin answering. `AN_PLUGINS` mounts a module of canned answers for
# each name: real plugins are Swift and Java, and there is neither of the two
# here, but the path —registry, call, promise— is the same one.
WITH="$(AN_PLUGINS='{"clipboard":{"read":"test text","write":null,"hasText":true}}' \
  cargo run -q -p an-bridge --example headless -- build/bundle/clipboard/main.js 6 2>&1)"
contains "$WITH" '(plugin de mentira|fake plugin|stub plugin): clipboard' \
  'the module is registered under the name from its package.json'
contains "$WITH" 'on the clipboard: test text' "what the plugin answered reaches the screen"
contains "$WITH" '"copied"' 'and the tap wrote: write resolved and triggered the re-read'

# With no plugin: the promise is rejected and it shows. This is the half that
# matters most —a method that swallowed the call would leave this same screen
# with a dash on it and no hint at all as to why.
#
# The pattern is short because the dump truncates node text at forty characters,
# and the prefix the example writes eats twenty-nine of them: only eleven of the
# bridge's own message survive. Those eleven come from a crate translated on
# another branch, so both openings are accepted.
WITHOUT="$(cargo run -q -p an-bridge --example headless -- build/bundle/clipboard/main.js 4 2>&1)"
contains "$WITHOUT" 'the clipboard failed: Error: (no hay ning|no native|there is no)' \
  'with no plugin the promise is rejected saying the module does not exist'

# A method the plugin does not declare is not swallowed either.
OTHER="$(AN_PLUGINS='{"clipboard":{"read":"something"}}' \
  cargo run -q -p an-bridge --example headless -- build/bundle/clipboard/main.js 4 2>&1)"
contains "$OTHER" 'the clipboard failed: Error: (el plugin|the plugin)' \
  'a method the plugin does not serve rejects the promise'

# ── 4. That it really compiles ──────────────────────────────────────────────
#
# None of the above touches Swift or Java. Building the .app and the APK without
# installing them does: `swiftc` compiles the plugin alongside the shell,
# `javac` likewise, and both have to see the registry `an` has just generated.
if cargo an ios examples/clipboard --no-launch >/dev/null 2>&1; then
  ok 'swiftc compiles the plugin and its registry into the .app'
else
  ko 'the .app with the plugin never got built'
  cargo an ios examples/clipboard --no-launch 2>&1 | tail -20
fi
GENERATED="build/ios/generated/AnGeneratedPlugins.swift"
if [ -f "$GENERATED" ]; then
  contains "$(cat "$GENERATED")" 'AnPluginRegistry.register\("clipboard", AnClipboardPlugin\(\)\)' \
    'the iOS registry links the manifest name to the Swift type'
else
  ko 'the iOS registry was not generated'
fi

# The build goes to a file and not inside a `$(...)` with its output covered up.
# With `set -e` and `pipefail`, an assignment like that never reaches the `ko`
# branch: the build failure kills the script on that very line, and since its
# output went to /dev/null, `check-all` was left exiting with 1 without a single
# line to read. That is what happens when `vendor/android` is missing.
APK_LOG="$(mktemp)"
if cargo an android examples/clipboard --no-launch >"$APK_LOG" 2>&1; then
  APK="$(tail -1 "$APK_LOG")"
else
  APK=""
fi
if [ -n "$APK" ] && [ -f "$APK" ]; then
  ok 'javac compiles the plugin and its registry into the APK'
else
  ko 'the APK with the plugin never got built'
  tail -20 "$APK_LOG"
fi
rm -f "$APK_LOG"
GENERATED="build/android/gen-plugins/dev/angularnative/AnGeneratedPlugins.java"
if [ -f "$GENERATED" ]; then
  contains "$(cat "$GENERATED")" \
    'AnPluginRegistry.register\("clipboard", new dev.angularnative.plugins.ClipboardPlugin\(\)\);' \
    'the Android registry links the manifest name to the Java class'
else
  ko 'the Android registry was not generated'
fi

exit "$fail"
