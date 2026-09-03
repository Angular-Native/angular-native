#!/usr/bin/env bash
# Plugin permissions: that they merge, that a clash stops the build, and that
# what was merged ends up inside the `.app` and the APK.
#
# It covers what can be checked without any device:
#
#   1. A plugin declares an `Info.plist` key and it ends up in the `.app`'s
#      plist. It is the half that keeps Face ID from killing the app on the very
#      first attempt.
#   2. Two plugins asking for the same key with the **same** value do not clash:
#      they say the same thing and it is written once. It is what really happens
#      between biometrics and keychain.
#   3. Two plugins asking for it with **different** values stop the build, and
#      the message names both packages and both values.
#   4. The same on Android with `uses-feature`, where the clash is
#      `android:required`. Permissions cannot clash and that is checked too.
#   5. A value that cannot be merged —a nested dictionary— is rejected rather
#      than half slipping through.
#   6. The entitlements end up inside the binary, in `__TEXT,__entitlements`,
#      which is what lets the keychain store anything.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0
ok() { echo "  ok   $1"; }
ko() { echo "  FAIL $1"; fail=1; }

contains() {
  if grep -qF -e "$2" <<<"$1"; then ok "$3"; else ko "$3"; fi
}

echo "== plugin permissions"

FIXTURE="$ROOT/build/permissions-fixture"
rm -rf "$FIXTURE"

# Sets up a fake app with two fake plugins. It is done under `build/` and not in
# the repo's workspaces: what is being tested is dependency resolution, and
# `node_modules` inside the app is where `an` looks first, just like Node.
#
#   $1 the app's directory   $2 the first one's angularNative JSON
#   $3 the second one's angularNative JSON (empty: there is only one)
setup() {
  local app="$FIXTURE/$1"
  mkdir -p "$app/node_modules/@fixture/one/native/ios" \
           "$app/node_modules/@fixture/one/native/android" \
           "$app/node_modules/@fixture/two/native/ios" \
           "$app/node_modules/@fixture/two/native/android"
  echo '// one source, so the directory is not empty' \
    | tee "$app/node_modules/@fixture/one/native/ios/One.swift" \
          "$app/node_modules/@fixture/one/native/android/One.java" \
          "$app/node_modules/@fixture/two/native/ios/Two.swift" \
          "$app/node_modules/@fixture/two/native/android/Two.java" >/dev/null
  echo '{}' >"$app/tsconfig.json"
  printf '%s' "$2" >"$app/node_modules/@fixture/one/package.json"
  if [ -n "${3:-}" ]; then
    printf '%s' "$3" >"$app/node_modules/@fixture/two/package.json"
    cat >"$app/package.json" <<'JSON'
{ "name": "@fixture/app", "private": true,
  "dependencies": { "@fixture/one": "0.0.1", "@fixture/two": "0.0.1" } }
JSON
  else
    rm -rf "$app/node_modules/@fixture/two"
    cat >"$app/package.json" <<'JSON'
{ "name": "@fixture/app", "private": true,
  "dependencies": { "@fixture/one": "0.0.1" } }
JSON
  fi
}

manifest() { # $1 module name  $2 plist text  $3 the uses-feature required flag
  cat <<JSON
{ "name": "@fixture/$1", "version": "0.0.1",
  "angularNative": {
    "module": "$1",
    "ios": { "sources": "native/ios", "register": "P$1",
             "plist": { "NSFaceIDUsageDescription": "$2" } },
    "android": { "sources": "native/android", "register": "dev.fixture.P$1",
                 "manifest": {
                   "uses-permission": ["android.permission.USE_BIOMETRIC"],
                   "uses-feature": { "android.hardware.fingerprint": $3 } } } } }
JSON
}

# ── 1 and 2. Same key, same value: not a clash ──────────────────────────────
setup agree "$(manifest one 'To get in.' false)" "$(manifest two 'To get in.' false)"
if AGREE="$(cargo an plugins build/permissions-fixture/agree --platform ios 2>&1)"; then
  ok 'two plugins asking for the same key with the same value do not clash'
else
  ko 'two plugins saying the same thing should not stop the build'
  echo "$AGREE" | tail -5
fi
if cargo an plugins build/permissions-fixture/agree --platform android >/dev/null 2>&1; then
  ok 'and the same with the Android manifest'
else
  ko 'the Android manifest should not clash either'
fi

# ── 3. Same key, different values: it stops ─────────────────────────────────
setup clash "$(manifest one 'To get in.' false)" "$(manifest two 'Something else.' false)"
if CLASH="$(cargo an plugins build/permissions-fixture/clash --platform ios 2>&1)"; then
  ko 'two plugins asking for the same key with different values should stop the build'
else
  ok 'two plugins asking for the same key with different values stop the build'
  contains "$CLASH" 'NSFaceIDUsageDescription' 'and the message says which key it is'
  contains "$CLASH" '@fixture/one' 'and which the first package is'
  contains "$CLASH" '@fixture/two' 'and which the second is'
  contains "$CLASH" 'To get in.' 'and what each one asked for'
  contains "$CLASH" 'Something else.' 'and what the other one asked for'
fi

# ── 4. The Android clash is `android:required` ──────────────────────────────
setup feature "$(manifest one 'Same.' true)" "$(manifest two 'Same.' false)"
if FEATURE="$(cargo an plugins build/permissions-fixture/feature --platform android 2>&1)"; then
  ko 'a feature asked for as required and as optional should stop the build'
else
  ok 'a feature asked for as required and as optional stops the build'
  contains "$FEATURE" 'android.hardware.fingerprint' 'and the message says which one it is'
  contains "$FEATURE" 'android:required' 'and what they cannot agree on'
fi
# The same pair does not clash on iOS: there they ask for the same thing. That an
# Android clash does not leak into the iOS build is the other half of the check.
if cargo an plugins build/permissions-fixture/feature --platform ios >/dev/null 2>&1; then
  ok 'and that same pair does not bother the iOS build, where they ask for the same thing'
else
  ko 'an Android clash has no business stopping the iOS build'
fi

# ── 5. A value that cannot be merged ────────────────────────────────────────
setup nested '{ "name": "@fixture/one", "version": "0.0.1",
  "angularNative": { "module": "one",
    "ios": { "sources": "native/ios", "register": "Pone",
             "plist": { "NSAppTransportSecurity": { "NSAllowsLocalNetworking": true } } } } }'
if NESTED="$(cargo an plugins build/permissions-fixture/nested --platform ios 2>&1)"; then
  ko 'a nested dictionary in the plist should be rejected'
else
  ok 'a nested dictionary in the plist is rejected rather than half slipping through'
  contains "$NESTED" 'NSAppTransportSecurity' 'and the message says which key it is'
fi

rm -rf "$FIXTURE"

# ── 6. And that what was merged really reaches the .app ─────────────────────
#
# This one does compile: it builds the example's `.app`, which depends on both
# real plugins. Both ask for `NSFaceIDUsageDescription` with the same text, which
# is exactly the case that has to go through without a word.
if cargo an ios examples/secrets --no-launch >/dev/null 2>&1; then
  ok 'the example .app is built with both plugins inside'
else
  ko 'the example .app never got built'
  cargo an ios examples/secrets --no-launch 2>&1 | tail -20
fi

PLIST="build/ios/AngularNative.app/Info.plist"
if [ -f "$PLIST" ]; then
  if VALUE="$(plutil -extract NSFaceIDUsageDescription raw -o - "$PLIST" 2>/dev/null)"; then
    ok "the plugin's key ended up in the .app's Info.plist"
    # This text is declared in the plugins' own package.json, under packages/.
    contains "$VALUE" 'To check it is you before showing what is stored.' \
      'and with the text the plugin declared'
  else
    ko 'NSFaceIDUsageDescription never reached the .app Info.plist'
  fi
  # And the shell's own is still where it was: merging is not replacing.
  if plutil -extract CFBundleExecutable raw -o - "$PLIST" >/dev/null 2>&1; then
    ok "and what the project's plist already carried is still there"
  else
    ko 'merging the plist ran over what was already there'
  fi
else
  ko 'the .app was not built'
fi

# The entitlements go inside the binary, not in the signature: `codesign -d` does
# not see them, but the `__TEXT,__entitlements` section is there and carries the
# keychain group.
BIN="build/ios/AngularNative.app/AngularNative"
if [ -f "$BIN" ]; then
  if otool -s __TEXT __entitlements "$BIN" 2>/dev/null | grep -q "__entitlements"; then
    ok 'the binary carries the __TEXT,__entitlements section'
    # `otool`'s dump comes in four-byte words and byte-swapped, so the text is
    # looked for in the binary as it stands rather than reassembled.
    for needle in keychain-access-groups dev.angularnative.playground; do
      if python3 -c 'import sys; sys.exit(0 if sys.argv[2].encode() in open(sys.argv[1],"rb").read() else 1)' \
           "$BIN" "$needle"; then
        ok "and the binary carries $needle inside it"
      else
        ko "the binary does not carry $needle"
      fi
    done
  else
    ko 'the binary does not carry the entitlements, and without them the keychain answers -34018'
  fi
else
  ko 'there is no binary to look at'
fi

exit "$fail"
