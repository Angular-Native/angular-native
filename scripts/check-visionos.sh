#!/usr/bin/env bash
# visionOS: that the headset app mounts in a window that is not a screen, that it
# does not paint over the system's glass, and that the scene manifest still says
# the same thing as the shell.
#
# The cross-compilation goes last because it needs nightly:
# `aarch64-apple-visionos-sim` is a tier 3 target and does not ship a precompiled
# `std`. If nightly is missing, this reports it and does not fail.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0

echo "== visionOS"

check() {
  if [ "$1" = "ok" ]; then
    echo "  ok   $2"
  else
    echo "  FAIL $2"
    fail=1
  fi
}

# 1. The headset app, mounted at the size the shell asks the window for when it
#    opens. It is not a screen: the user pulls the corner and it changes, and the
#    real viewport arrives through `viewDidLayoutSubviews`. That is why nothing
#    on this screen is in fixed points.
cargo an build examples/hello-vision >/dev/null
OUTPUT="$(AN_VIEWPORT=1280x720 cargo run -q -p an-bridge --example headless -- \
  build/bundle/hello-vision/main.js 6 2>&1)"

grep_check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    check ok "$2"
  else
    check no "$2"
  fi
}

grep_check 'View#1 \[0,0 1280x720\]' "the headset window measures what the shell asked for"
grep_check 'View#7 \[40,193 1200x160\]' "the row of cards takes up the width of the window"
grep_check 'View#13 \[399,0 ' 'the cards share the width with flexGrow, no fixed points'
grep_check '"you chose: one"' 'the press reached JS and the signal was recomputed'

# The background, which on visionOS is the decision that shows the most.
#
# The window already brings one: the glass the system draws, with its blur and
# its shadow over the real room. A `[backgroundColor]` on the top container would
# cover it entirely and the app would be an opaque slab floating in the living
# room. What is checked here is that the template's root container still paints
# nothing.
if grep -qE 'View#2 \[0,0 1280x720\]\s*$' <<<"$OUTPUT"; then
  check ok "the template's root paints no background: the system glass shows"
else
  check no "the template's root paints a background and would cover the window's glass"
fi

# 2. The shell's `Info.plist` against what the build expects of it.
PLIST="shells/visionos/Resources/Info.plist"
plist_check() {
  read_value="$(plutil -extract "$1" raw -o - "$PLIST" 2>/dev/null || echo '<no key>')"
  if [ "$read_value" = "$2" ]; then
    check ok "$PLIST: $1 is $2"
  else
    check no "$PLIST: $1 is \"$read_value\" and the build expects \"$2\""
  fi
}
plist_check CFBundleExecutable AngularNativeVision
plist_check CFBundleIdentifier dev.angularnative.playground.vision
# 7 is the headset.
if grep -qx 7 <<<"$(plutil -extract UIDeviceFamily.0 raw -o - "$PLIST" 2>/dev/null || true)"; then
  check ok "$PLIST: UIDeviceFamily is 7, the headset"
else
  check no "$PLIST: UIDeviceFamily is not 7"
fi
if grep -q 'Family::VisionOs => ("Vision", ".vision")' crates/an-cli/src/ios.rs; then
  check ok 'Family::suffix sets Vision and .vision, which is what the plist says'
else
  check no 'Family::suffix no longer sets Vision and .vision, and the plist still says so'
fi

# 3. The scene delegate, which is this family's own silent failure.
#
#    On visionOS there is no `UIScreen`, so the window can only come out of a
#    `UIWindowScene`, and the system only creates one if the manifest says who
#    handles it. That name is the Objective-C one, the one `@objc` gives the
#    Swift class. If they stop matching, the scene connects, nobody creates the
#    window, and the app starts up black without a single error.
SCENE="$(plutil -extract \
  UIApplicationSceneManifest.UISceneConfigurations.UIWindowSceneSessionRoleApplication.0.UISceneDelegateClassName \
  raw -o - "$PLIST" 2>/dev/null || echo '<no key>')"
if grep -q "@objc($SCENE)" shells/ios/Sources/SceneDelegate.swift; then
  check ok "the manifest asks for $SCENE and the shell declares @objc($SCENE)"
else
  check no "the manifest asks for $SCENE and the shell does not declare that class: the app would start up black"
fi

# 4. The cross-compilation.
if ! grep -q '^nightly' <<<"$(rustup toolchain list 2>/dev/null || true)"; then
  echo "  --   cross-compilation skipped: the nightly toolchain is missing"
  # One command, with the component in it: two lines invite installing the first
  # and reading the second as optional, and nightly without `rust-src` skips here
  # all the same. It is the line CI runs.
  echo "       rustup toolchain install nightly --component rust-src"
elif ! grep -q 'rust-src (installed)' <<<"$(rustup component list --toolchain nightly 2>/dev/null || true)"; then
  echo "  --   cross-compilation skipped: rust-src is missing from nightly"
  echo "       rustup component add rust-src --toolchain nightly"
else
  # The output is kept rather than thrown away. `-Z build-std` compiles `std`
  # first, so a failure here is as often the toolchain as the crate, and
  # `2>/dev/null` cannot tell the two apart — it prints FAIL and nothing to act
  # on.
  XLOG="$(mktemp)"
  if XROS_DEPLOYMENT_TARGET=1.0 cargo +nightly build --quiet -Z build-std=std,panic_abort \
    -p an-ios --target aarch64-apple-visionos-sim >"$XLOG" 2>&1; then
    check ok 'an-ios for aarch64-apple-visionos-sim'
  else
    check no 'an-ios does not cross-compile for aarch64-apple-visionos-sim'
    tail -30 "$XLOG" | sed 's/^/       /'
  fi
fi

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT"
  exit 1
fi
