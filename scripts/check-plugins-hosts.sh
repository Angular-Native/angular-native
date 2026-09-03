#!/usr/bin/env bash
# Plugins on the two hosts that used to refuse them: macOS and watchOS.
#
# `check-plugins.sh` covers the mechanism on iOS and Android — discovery, the
# missing platform owning up, the call going out and coming back. This covers
# what is different about the other two, and every one of the four things below
# is different for a reason that cost something to find out:
#
#   1. A plugin can now say *why* it cannot cover a platform, and the refusal
#      quotes it. "plugin-biometrics does not cover watchOS" is something anybody
#      could read off the package.json; "a watch has no biometric sensor and
#      LocalAuthentication marks the policy API_UNAVAILABLE(watchos)" is not.
#   2. Entitlements matter on the Mac in a way they do not on the phone, and an
#      ad-hoc signature cannot carry the ones the system grants rather than the
#      ones the app imposes on itself. An `.app` that carries one anyway is
#      killed at launch with a bare `Killed: 9`.
#   3. The registry has to actually be generated and actually compile, into a
#      `swiftc` invocation that is not the phone's.
#   4. And the whole thing has to work on a running Mac: a real press, a real
#      NSPasteboard, the answer back on screen. That is the only part no static
#      check can stand in for.
#
# The watch's own end-to-end run — build the .app, put it on the simulator, read
# the round trip off a screenshot — needs nightly with rust-src and takes minutes
# to build `std` from source, so it is behind `AN_CHECK_WATCH_APP=1` and says so
# when it skips. Everything else here runs every time.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0
ok() { echo "  ok   $1"; }
ko() { echo "  FAIL $1"; fail=1; }

# Check that some output contains a pattern. The `-e` is not redundant: there are
# patterns starting with a dash, and without it grep takes them for options.
contains() {
  if grep -qE -e "$2" <<<"$1"; then ok "$3"; else ko "$3"; fi
}

lacks() {
  if grep -qE -e "$2" <<<"$1"; then ko "$3"; else ok "$3"; fi
}

echo "== plugins on macOS and watchOS"

# ── 1. What each plugin says about each host ────────────────────────────────
LIST="$(cargo an plugins examples/clipboard 2>&1)"
contains "$LIST" 'ios \+ android \+ macos' 'the clipboard covers macOS and says so'
contains "$LIST" 'no watchos: watchOS has no system pasteboard' \
  'and the listing carries its reason for not covering the watch'

LIST="$(cargo an plugins examples/secrets 2>&1)"
contains "$LIST" 'keychain .*ios \+ android \+ macos \+ watchos' \
  'the keychain covers all four platforms'
contains "$LIST" 'no watchos: a watch has no biometric sensor' \
  'and biometrics says why the watch is not one of its'

# ── 2. What passes and what stops the build ────────────────────────────────
for app_platform in "examples/clipboard macos" "examples/secrets macos" \
  "examples/watch-secrets watchos" "examples/watch-secrets macos"; do
  set -- $app_platform
  if cargo an plugins "$1" --platform "$2" >/dev/null 2>&1; then
    ok "$1 passes the $2 check"
  else
    ko "$1 should pass the $2 check"
  fi
done

# The clipboard on a watch. It is not an unfinished plugin, and the message has
# to say so: telling somebody to go and write the missing half would be telling
# them to write something that cannot exist.
if NEGATIVE="$(cargo an plugins examples/clipboard --platform watchos 2>&1)"; then
  ko 'the clipboard should not be buildable for watchOS'
else
  ok 'the clipboard is refused for watchOS'
  contains "$NEGATIVE" 'says it cannot: watchOS has no system pasteboard' \
    "and the refusal quotes the plugin's own reason"
  contains "$NEGATIVE" 'These are not unfinished halves' \
    'and does not ask anybody to go and write the missing half'
  lacks "$NEGATIVE" 'does not load plugins yet' \
    'and no longer claims the watch loads no plugins'
fi

# And the mixed app: one plugin the watch can run, one it cannot.
if NEGATIVE="$(cargo an plugins examples/secrets --platform watchos 2>&1)"; then
  ko 'examples/secrets should not be buildable for watchOS'
else
  ok 'examples/secrets is refused for watchOS because of biometrics'
  contains "$NEGATIVE" '@angular-native/plugin-biometrics' \
    'and the refusal names which of the two plugins it was'
  contains "$NEGATIVE" 'API_UNAVAILABLE\(watchos\)' \
    'and cites the header that settles it'
  lacks "$NEGATIVE" 'plugin-keychain \(module' \
    'and does not blame the plugin that does cover the watch'
fi

# ── 3. A plugin that says nothing, and the manifest's own refusals ─────────
#
# With fake plugins under `build/`, so as not to put sham packages into the
# repo's workspaces. `node_modules` inside the app is exactly where Node —and
# `an`— look first.
FIXTURE="$ROOT/build/plugins-hosts-fixture"
fixture() { # <package name> <the angularNative body, without the module>
  rm -rf "$FIXTURE"
  mkdir -p "$FIXTURE/app/node_modules/@fixture/$1/native/macos"
  cat >"$FIXTURE/app/package.json" <<JSON
{
  "name": "@fixture/app-$1",
  "private": true,
  "dependencies": { "@fixture/$1": "0.0.1" }
}
JSON
  # `an` only asks that it exists to recognise the directory as an app.
  echo '{}' >"$FIXTURE/app/tsconfig.json"
  cat >"$FIXTURE/app/node_modules/@fixture/$1/package.json" <<JSON
{
  "name": "@fixture/$1",
  "version": "0.0.1",
  "angularNative": { "module": "${1//-/}", $2 }
}
JSON
  echo '// only so the directory has a source in it' \
    >"$FIXTURE/app/node_modules/@fixture/$1/native/macos/Fixture.swift"
}

# A plugin with a macOS half and nothing at all about the watch. It has not
# decided; the message has to offer all three ways out.
fixture mac-only '"macos": { "sources": "native/macos", "register": "FixturePlugin" }'
if SILENT="$(cargo an plugins build/plugins-hosts-fixture/app --platform watchos 2>&1)"; then
  ko 'a plugin with no watchOS half should not pass the watchOS check'
else
  ok 'a plugin with no watchOS half is refused'
  contains "$SILENT" 'only brings macos' 'and the message says what it does bring'
  contains "$SILENT" 'angularNative.watchos.unsupported' \
    'and offers saying why it cannot as one of the ways out'
fi

# `sources` and `unsupported` together: it is one or the other, and guessing
# which the author meant is not the manifest reader's job.
fixture both '"watchos": { "sources": "native/macos", "register": "F", "unsupported": "no" }'
if BOTH="$(cargo an plugins build/plugins-hosts-fixture/app 2>&1)"; then
  ko 'sources and unsupported together should be refused'
else
  ok 'a platform section with both sources and unsupported is refused'
  contains "$BOTH" 'It is one or the other' 'and says why it cannot be both'
fi

# An empty reason. The reason is the whole point of the field.
fixture empty '"watchos": { "unsupported": "" }'
if EMPTY="$(cargo an plugins build/plugins-hosts-fixture/app 2>&1)"; then
  ko 'an empty unsupported should be refused'
else
  ok 'an empty unsupported reason is refused'
  contains "$EMPTY" 'says less than leaving the section out' 'and says why'
fi
rm -rf "$FIXTURE"

# ── 4. That it really compiles, and what ends up in the signature ──────────
BUILD_LOG="$(mktemp)"
# The entitlements. Two things have to be true at once and they pull in opposite
# directions: the engine's two have to be there or the app is killed on its first
# allocation, and the keychain's must **not** be there in an ad-hoc build or the
# app is killed before that. Both kills print the same `Killed: 9`.
if cargo an macos examples/secrets --no-launch >"$BUILD_LOG" 2>&1; then
  ok 'the macOS .app with two plugins builds'
  contains "$(cat "$BUILD_LOG")" 'keychain-access-groups .* is left out of this build' \
    'and the entitlement an ad-hoc signature cannot carry is dropped, out loud'
  SIGNED="$(codesign -d --entitlements - build/macos/AngularNativeMac.app 2>&1)"
  contains "$SIGNED" 'com.apple.security.cs.allow-jit' \
    'the signature carries the entitlement QuickJS cannot start without'
  lacks "$SIGNED" 'keychain-access-groups' \
    'and does not carry the one that would have it killed at launch'
else
  ko 'the macOS .app with two plugins never got built'
  tail -20 "$BUILD_LOG"
fi

# The clipboard goes last of the two on purpose: there is one `build/macos`, so
# whichever was built last is the `.app` the run below drives, and the run needs
# this one.
if cargo an macos examples/clipboard --no-launch >"$BUILD_LOG" 2>&1; then
  ok 'swiftc compiles the plugin and its registry into the macOS .app'
else
  ko 'the macOS .app with the plugin never got built'
  tail -20 "$BUILD_LOG"
fi
GENERATED="build/macos/generated/AnGeneratedPlugins.swift"
if [ -f "$GENERATED" ]; then
  contains "$(cat "$GENERATED")" 'AnPluginRegistry.register\("clipboard", AnClipboardPlugin\(\)\)' \
    'the macOS registry links the manifest name to the Swift type'
else
  ko 'the macOS registry was not generated'
fi
rm -f "$BUILD_LOG"

# ── 5. A plugin call that resolves on a running Mac ────────────────────────
#
# Everything above is the plumbing. This is the app: a real press on a real
# window, `NSPasteboard.general` written and read back, and the answer on screen.
# `AN_DUMP_TEXT=1` logs the strings the NSTextFields are really showing, which is
# what turns "the window is not black" into "the plugin answered this".
#
# The pointer is not moved and the app does not come to the front (see
# `AppDelegate`), so this does not interrupt whoever is running it. What it does
# touch is the pasteboard, which belongs to them — so it is put back afterwards.
BEFORE="$(pbpaste 2>/dev/null || true)"
restore_pasteboard() {
  if [ -n "$BEFORE" ]; then printf '%s' "$BEFORE" | pbcopy; fi
}
trap restore_pasteboard EXIT

printf 'a phrase this check put there' | pbcopy
RUN_LOG="$(mktemp)"
# The press lands on the "copy" button of examples/clipboard: the first of the
# three in the row, in window points. The app is 720 wide with 20 of padding and
# three buttons sharing the rest, so the first one's middle is near x=115; the
# row sits at y=242 under the heading, the description and the phrase box.
AN_SCREENSHOT="$(mktemp -t an-clipboard).png" AN_DUMP_TEXT=1 \
  AN_SCREENSHOT_FRAMES=120 AN_SCREENSHOT_PRESS=115,242 \
  build/macos/AngularNativeMac.app/Contents/MacOS/AngularNativeMac \
  >"$RUN_LOG" 2>&1 || true
SAID="$(sed -n 's/.*\[text\] //p' "$RUN_LOG")"

# Careful with the order these are read in. `hasText` runs at startup and reports
# on the phrase this check copied in; `write` and `read` run because of the
# press. All three are the plugin, and all three are `NSPasteboard`.
contains "$SAID" '^something is copied$|^copied$' \
  'a plugin call resolves on a running Mac window'
contains "$SAID" '^copied$' 'clipboard.write resolved after a real press'
contains "$SAID" '^on the clipboard: hello from angular-native$' \
  'and clipboard.read brought back what write had just put there'
if [ "$(pbpaste)" = "hello from angular-native" ]; then
  ok "and the Mac's own pasteboard really holds it"
else
  ko "the Mac's pasteboard does not hold what the plugin says it wrote"
  echo "      pbpaste says: $(pbpaste)"
fi
rm -f "$RUN_LOG"
restore_pasteboard
trap - EXIT

# ── 6. The watch, end to end ───────────────────────────────────────────────
#
# Behind a flag: `aarch64-apple-watchos-sim` is a tier 3 target with no
# precompiled `std`, so this builds one from source with nightly and takes
# minutes. What it proves is the thing nothing else can — six plugin calls
# resolving through the new registry on a real watch simulator.
#
# `examples/watch-secrets` does its whole round trip on startup and writes the
# verdict in its first line, because tapping a watch simulator needs the Mac's
# own mouse over the Simulator window and a check does not always have a desktop
# to borrow. See the example.
WATCH_ID="dev.angularnative.playground.watchkitapp"
if [ "${AN_CHECK_WATCH_APP:-}" = "1" ]; then
  WATCH_LOG="$(mktemp)"
  if cargo an watchos examples/watch-secrets >"$WATCH_LOG" 2>&1; then
    ok 'the watch .app with the keychain plugin builds, installs and launches'
    contains "$(cat "$WATCH_LOG")" 'plugin keychain' \
      'and the keychain Swift went into the same swiftc as the shell'
    # Now again, with the console attached. The app writes its verdict both on
    # screen and through `console.log`, and this is the half a script can read:
    # `simctl io … screenshot` takes the picture but nothing reads a sentence
    # out of it, and tapping the watch needs the Mac's own mouse over the
    # Simulator window, which a check does not always have a desktop for.
    CONSOLE="$(mktemp)"
    xcrun simctl terminate booted "$WATCH_ID" >/dev/null 2>&1 || true
    (xcrun simctl launch --console-pty booted "$WATCH_ID" >"$CONSOLE" 2>&1 &)
    # QuickJS starts, Angular boots, and only then does the round trip run.
    # Forty seconds is generous and it stops as soon as the line shows up.
    VERDICT=""
    for _ in $(seq 40); do
      sleep 1
      VERDICT="$(sed -n 's/.*\(round trip[^\r]*\)/\1/p' "$CONSOLE" | tail -1)"
      [ -n "$VERDICT" ] && break
    done
    xcrun simctl terminate booted "$WATCH_ID" >/dev/null 2>&1 || true
    if [ -n "$VERDICT" ]; then
      contains "$VERDICT" 'stored, read and deleted' \
        'the keychain round trip resolves on the watch simulator'
      contains "$VERDICT" 'biometrics refused' \
        'and requireBiometrics comes back refused with nothing stored'
    else
      ko 'the watch .app never said how its round trip went'
      tail -20 "$CONSOLE"
    fi
    rm -f "$CONSOLE"
  else
    ko 'the watch .app with the keychain plugin never got built'
    tail -20 "$WATCH_LOG"
  fi
  rm -f "$WATCH_LOG"
else
  echo "  --   the watch simulator run is skipped (AN_CHECK_WATCH_APP=1 runs it;"
  echo "       it builds std from source with nightly and takes minutes)"
fi

exit "$fail"
