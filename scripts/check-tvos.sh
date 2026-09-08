#!/usr/bin/env bash
# tvOS: that the television app mounts with television measurements, that what
# the SDK does not ship is still said in the three places it is said, and that
# the core cross-compiles.
#
# The cross-compilation is kept apart, and last, because it needs nightly:
# `aarch64-apple-tvos-sim` is a tier 3 target and does not ship a precompiled
# `std`, so it has to be built on the spot with `-Z build-std`. If nightly is
# missing, this reports it and does not fail: the rest of the checks have no
# reason to fall over because somebody is short a toolchain.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0

echo "== tvOS"

check() {
  if [ "$1" = "ok" ]; then
    echo "  ok   $2"
  else
    echo "  FAIL $2"
    fail=1
  fi
}

# 1. The television app, mounted with an Apple TV's viewport: 1920x1080 points,
#    not an iPhone's 393. Without this a change in the primitives could leave it
#    unpainted and nobody would find out until opening the simulator.
cargo an build examples/hello-tv >/dev/null
OUTPUT="$(AN_VIEWPORT=1920x1080 cargo run -q -p an-bridge --example headless -- \
  build/bundle/hello-tv/main.js 6 2>&1)"

grep_check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    check ok "$2"
  else
    check no "$2"
  fi
}

grep_check 'View#1 \[0,0 1920x1080\]' "the television's viewport is 1920x1080"
# Overscan: the edges of a TV set are cropped, and Apple asks for 90 points at
# the sides and 60 at the top. That is set by the template, not by the framework,
# so if somebody removes it nothing fails here: it goes off the screen in
# somebody else's house.
grep_check 'Text#3 \[90,60 ' 'the overscan margins are 90 x 60'
grep_check 'Button#7 \[90,286 760x88\]' 'the first system button is mounted'
grep_check 'Button#8 \[90,402 760x88\]' 'the second system button is mounted'
grep_check 'View#9 \[90,518 760x88\]' 'the focusable an-view is mounted and is not a control'
grep_check 'title=top — pressed 1 times' 'the press reached JS and the signal was recomputed'

# 2. What the tvOS SDK does not ship, said in all three places.
#
#    `family.rs` is the real list —it is the SDK's annotation brought into
#    Rust—, and two things come out of it that can drift apart without anything
#    failing: the warning written to the log and the row of the table in
#    docs-site/src/content/docs/platforms/tvos.md. Drifting apart is exactly what
#    leaves somebody looking for a control that is never going to appear.
#    The cfg range is taken inside `missing_kind` and not over the whole file.
#    `family.rs` has more than one `#[cfg(target_os = "tvos")]` — `unsupported_event`
#    is cfg'd per family too — and a range that starts at each of them swept up the
#    kinds named in event refusals as though they were primitives the SDK does not
#    ship.
MISSING_KIND="$(awk '/^pub fn missing_kind/,/^\}/' crates/an-ios/src/family.rs)"
KINDS="$(sed -n '/#\[cfg(target_os = "tvos")\]/,/#\[cfg(not(target_os = "tvos"))\]/p' \
  <<<"$MISSING_KIND" | grep -oE 'NodeKind::[A-Za-z]+' | sed 's/NodeKind:://' | sort -u)"
if [ -z "$KINDS" ]; then
  check no 'family.rs declares which primitives do not exist on tvOS'
else
  check ok "family.rs declares $(wc -l <<<"$KINDS" | tr -d ' ') primitives tvOS does not ship"
  for kind in $KINDS; do
    # The project rule, from PascalCase to the tag: WebView is an-web-view. It is
    # the same one that goes from the core to the template, so there is no
    # translation table that can fall behind.
    tag="an-$(sed -E 's/([a-z0-9])([A-Z])/\1-\2/g' <<<"$kind" | tr '[:upper:]' '[:lower:]')"
    if grep -q -- "$tag" docs-site/src/content/docs/platforms/tvos.md; then
      check ok "docs-site/src/content/docs/platforms/tvos.md says what happens with $tag"
    else
      check no "docs-site/src/content/docs/platforms/tvos.md does not mention $tag, which family.rs gives as unavailable"
    fi
  done
fi

# 3. The shell's `Info.plist` against what the build expects of it.
#
#    The name and the identifier carry the family suffix, and the build compares
#    them with `plutil` before compiling anything. If the plist and
#    `Family::suffix` stop saying the same thing, the app installs and disappears
#    when opened: the system looks for an executable that is not there.
PLIST="shells/tvos/Resources/Info.plist"
plist_check() {
  read_value="$(plutil -extract "$1" raw -o - "$PLIST" 2>/dev/null || echo '<no key>')"
  if [ "$read_value" = "$2" ]; then
    check ok "$PLIST: $1 is $2"
  else
    check no "$PLIST: $1 is \"$read_value\" and the build expects \"$2\""
  fi
}
plist_check CFBundleExecutable AngularNativeTV
plist_check CFBundleIdentifier dev.angularnative.playground.tv
# 3 is the Apple TV. Without this key `simctl` installs something it then does
# not know how to launch, and the error arrives much later and in another
# language.
if grep -qx 3 <<<"$(plutil -extract UIDeviceFamily.0 raw -o - "$PLIST" 2>/dev/null || true)"; then
  check ok "$PLIST: UIDeviceFamily is 3, the Apple TV"
else
  check no "$PLIST: UIDeviceFamily is not 3"
fi
if grep -q 'Family::TvOs => ("TV", ".tv")' crates/an-cli/src/ios.rs; then
  check ok 'Family::suffix sets TV and .tv, which is what the plist says'
else
  check no 'Family::suffix no longer sets TV and .tv, and the plist still says so'
fi

# 4. WebKit is not part of the tvOS SDK. The whole module has to go out of the
#    binary, and not only because of the class: its `#[link(name = "WebKit")]`
#    would make the link look for a framework that is not in that SDK, and that
#    does not show up until the `swiftc` at the end.
if grep -q 'target_os = "tvos"' <<<"$(grep -B1 '^mod web;' crates/an-ios/src/lib.rs || true)"; then
  check no 'the web module is still compiled for tvOS, and WebKit is not in its SDK'
else
  check ok 'the web module goes out of the binary on tvOS: WebKit is not in its SDK'
fi

# 5. The button, which is where the silent bug was.
#
#    `TouchUpInside` is what UIKit sends when a finger lifts off the screen, and
#    on a television there are no fingers: `PrimaryActionTriggered` arrives. With
#    the wrong event the button takes the focus, turns white, and pressing it
#    does nothing. There is no error to look at, so it is checked here.
#    The window is the one arm and not the union of them. `events.rs` has eight
#    `#[cfg(target_os = "tvos")]` sites; a `sed` range emits a window for each
#    and a grep over the concatenation only asks that the two strings appear
#    somewhere between them. Swapping the two arms leaves the television on
#    `TouchUpInside` and both strings still in the pile. So the arm is found by
#    the cfg line immediately above it, and read three lines on from there.
TV_BUTTON="$(awk '
  previous ~ /#\[cfg\(target_os = "tvos"\)\]/ && /\(NodeKind::Button, "press"\)/ { left = 3 }
  left > 0 { print; left-- }
  { previous = $0 }
' crates/an-ios/src/events.rs)"
if [ -z "$TV_BUTTON" ]; then
  check no 'the tvOS button no longer has a control event of its own'
elif grep -q 'PrimaryActionTriggered' <<<"$TV_BUTTON"; then
  check ok 'on tvOS the button listens for PrimaryActionTriggered, not TouchUpInside'
else
  check no 'on tvOS the button no longer listens for PrimaryActionTriggered'
fi

# 6. The cross-compilation, which is the expensive one and the one that may not
#    be available.
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
  if TVOS_DEPLOYMENT_TARGET=17.0 cargo +nightly build --quiet -Z build-std=std,panic_abort \
    -p an-ios --target aarch64-apple-tvos-sim >"$XLOG" 2>&1; then
    check ok 'an-ios for aarch64-apple-tvos-sim'
  else
    check no 'an-ios does not cross-compile for aarch64-apple-tvos-sim'
    tail -30 "$XLOG" | sed 's/^/       /'
  fi
fi

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT"
  exit 1
fi
