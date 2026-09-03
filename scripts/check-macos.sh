#!/usr/bin/env bash
# The macOS host: what has to be run to check it.
#
# The text checks —the inventory against the enum, the list of ignored props,
# the hot reload— are in `check-macos.py`. What is here is what is a process:
# compiling, building the `.app`, launching it and looking at what it painted.
#
# **And launching it can be done.** That is what separates this platform from
# the other four: there is no simulator to bring up and no device to look for,
# the app runs on the same machine that compiled it and terminates on its own.
# So the check here does not stop at "it cross-compiles": the app opens, mounts
# the tree and takes a screenshot of itself, and what is examined is that image.
#
# The screenshot is taken by the app and not by `screencapture` on purpose.
# Asking the system for the screen requires the recording permission, which is
# granted by hand and per application: a check that depends on that fails on the
# machine of anyone who has not granted it, and fails for a reason that has
# nothing to do with what was being checked. A view, on the other hand, knows how
# to draw itself into a bitmap without asking anyone's permission. See
# `shells/macos/Sources/Screenshot.swift`.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0

echo "== macOS"

# The text checks go first: they are instant and compile nothing, so if the
# inventory is wrong it is known before waiting for a link.
if ! python3 "$ROOT/scripts/check-macos.py" "$ROOT"; then
  fail=1
fi

if [ "$(uname -s)" != "Darwin" ]; then
  echo "  --   the rest is skipped: the macOS host only compiles on a Mac"
  exit "$fail"
fi

# 1. The inventory, from the inside. It opens no window: `support.rs` is outside
#    `cfg(target_os = "macos")` so it can be looked at from here.
if cargo test --quiet -p an-macos >/dev/null 2>&1; then
  echo "  ok   the inventory of primitives holds up"
else
  echo "  FAIL the an-macos tests do not pass"
  cargo test -p an-macos 2>&1 | tail -20
  fail=1
fi

# 2. The whole `.app`: core for aarch64-apple-darwin, AppKit shell linked with
#    swiftc and an ad-hoc signature. Without the signature, macOS kills the app
#    at the first piece of code QuickJS generates.
BUILD_LOG="$(mktemp)"
if cargo an macos --no-launch >"$BUILD_LOG" 2>&1; then
  echo "  ok   the .app is built and signed"
else
  echo "  FAIL the macOS .app does not build"
  tail -30 "$BUILD_LOG"
  rm -f "$BUILD_LOG"
  exit 1
fi
rm -f "$BUILD_LOG"

APP="$ROOT/build/macos/AngularNativeMac.app"

# 3. The shape of the bundle. A macOS `.app` is not flat like the iOS one, and a
#    badly built one is opened by the Finder and rejected by `open` without
#    saying why.
missing=()
for piece in Contents/Info.plist Contents/MacOS/AngularNativeMac Contents/Resources/main.js; do
  [ -e "$APP/$piece" ] || missing+=("$piece")
done
if [ "${#missing[@]}" -eq 0 ]; then
  echo "  ok   the bundle has its Info.plist, its executable and its main.js"
else
  echo "  FAIL the bundle is missing: ${missing[*]}"
  fail=1
fi

if codesign --verify --deep "$APP" >/dev/null 2>&1; then
  echo "  ok   the ad-hoc signature is valid"
else
  echo "  FAIL the .app is not signed: QuickJS would die generating code"
  fail=1
fi

# 4. And now the real one: launching it.
SHOT="$ROOT/build/macos/screenshot.png"
RUN_LOG="$(mktemp)"
rm -f "$SHOT"
if AN_SCREENSHOT="$SHOT" "$APP/Contents/MacOS/AngularNativeMac" >"$RUN_LOG" 2>&1; then
  echo "  ok   the app starts, mounts the tree and closes on its own"
else
  echo "  FAIL the app never mounted anything (exit code $?)"
  tail -30 "$RUN_LOG"
  fail=1
fi

# 5. That it painted. A PNG of the right size and entirely black weighs its
#    share and would pass any test that looks at the file size, so what is
#    examined is how many distinct colours there are: the app counts them as it
#    saves.
colours="$(sed -n 's/.*, \([0-9]*\) colours).*/\1/p' "$RUN_LOG" | tail -1)"
if [ -s "$SHOT" ] && [ -n "$colours" ] && [ "$colours" -gt 16 ]; then
  echo "  ok   the window painted the controls ($colours colours in the screenshot)"
else
  echo "  FAIL the screenshot came out blank or was not written"
  fail=1
fi

# 6. The noisy path, which is what holds up the house rule. The controls example
#    uses props AppKit cannot honour, and they have to come out on screen the
#    first time they arrive.
#
# The host is a crate and is being translated on its own branch, so the patterns
# that read its prose accept either language.
if grep -qE "no se aplica en macOS|does not apply on macOS" "$RUN_LOG"; then
  echo "  ok   what AppKit does not cover is said on arrival, not swallowed"
else
  echo "  FAIL not one discarded prop came out on screen: the warning does not work"
  fail=1
fi

# 7. And the reverse: nothing nobody has declared. An "unknown prop" is a prop
#    this host does not look at and that is not in IGNORED either, that is, an
#    oversight.
if grep -qE "prop desconocida|unknown prop" "$RUN_LOG"; then
  echo "  FAIL there are props the host does not look at and that are not declared:"
  grep -E "prop desconocida|unknown prop" "$RUN_LOG" | sed 's/^/       /'
  fail=1
else
  echo "  ok   no prop of the example is left without an owner"
fi

# 8. The real desktop: the pointer and the swipe.
#
# The two things this platform has and the other four do not, and both can be
# checked here *by running the app*, which is what separates macOS from the
# rest: there is no simulator to bring up and no device to look for.
#
# The pointer really moves. `CGWarpMouseCursorPosition` asks for no permission
# —it is not `CGEventPost`, which does require accessibility—, so what enters the
# `NSTrackingArea` is the mouse, and what comes out in the picture is the system
# area doing its job.
#
# The swipe cannot be provoked: the gesture is recognised by the system from two
# fingers on the trackpad and there is no way to ask it for one. What can be done
# is to come in through its own door —`swipeWithEvent:` on the view under the
# point— with the deltas it sends, and check everything that comes after. See
# `Screenshot.swift`.
BUILD_LOG="$(mktemp)"
if cargo an macos examples/desktop --no-launch >"$BUILD_LOG" 2>&1; then
  echo "  ok   the desktop example builds"
else
  echo "  FAIL the desktop example does not build"
  tail -20 "$BUILD_LOG"
  fail=1
fi
rm -f "$BUILD_LOG"

BIN="$APP/Contents/MacOS/AngularNativeMac"
SHOT_STILL="$ROOT/build/macos/desktop.png"
SHOT_HOVER="$ROOT/build/macos/desktop-hover.png"
DESK_LOG="$(mktemp)"
HOVER_LOG="$(mktemp)"

AN_SCREENSHOT="$SHOT_STILL" AN_SCREENSHOT_FRAMES=150 \
  AN_SCREENSHOT_SWIPE=360,600,-1,0 "$BIN" >"$DESK_LOG" 2>&1 || true

# The sign comes from `NSEvent.h`: "-1 for swipe right". If this line stops
# adding up, somebody changed the mapping, not that the gesture fails to arrive.
if grep -q "\[swipe\] right" "$DESK_LOG"; then
  echo "  ok   a swipe with deltaX -1 reaches the template as \"right\""
else
  echo "  FAIL the swipe never reached the template"
  fail=1
fi

# The pointer over the first card. The coordinates are those of the 720x820
# window the shell opens; if the example moves things around, they have to be
# moved here.
AN_SCREENSHOT="$SHOT_HOVER" AN_SCREENSHOT_FRAMES=150 \
  AN_SCREENSHOT_HOVER=97,200 "$BIN" >"$HOVER_LOG" 2>&1 || true

# A hover only happens if the window is actually under the pointer, and that
# needs the app to win the front. With a simulator or an emulator open, macOS
# hands the front to whoever asked last and this check would fail for a reason
# that has nothing to do with the code — it has already happened to three
# separate runs. So the precondition is checked first and reported as a skip,
# loudly and with its reason. A skip is not a pass: the run says so, and the
# same binary passes as soon as nothing else is fighting for the front.
if grep -q "\[hover\] inside pointer" "$HOVER_LOG"; then
  echo "  ok   the pointer enters the view and the template hears about it"
elif grep -q "frontmost=no" "$HOVER_LOG"; then
  echo "  skipped the (hover): the app never reached the front, so the pointer"
  echo "           was never on top of it. Close the simulators and try again."
else
  echo "  FAIL nobody received the (hover) with the mouse on top"
  fail=1
fi

# And that it also shows. A `(hover)` that arrives and changes nothing on screen
# is half the job: what has to be checked is that the tree was recomposed.
if [ -s "$SHOT_STILL" ] && [ -s "$SHOT_HOVER" ] \
  && ! cmp -s "$SHOT_STILL" "$SHOT_HOVER"; then
  echo "  ok   and the window changes with the mouse on top"
else
  echo "  FAIL the window looks the same with the mouse on top as without it"
  fail=1
fi

# 9. The map and the video, which are the two that were left.
#
# What is examined of the map is the image: `MKMapView` draws tiles and that
# pushes the colour count well above what an empty box gives. Of the video the
# image cannot be examined, and that is not glossed over: `AVPlayerView`
# composes its frames outside the view's drawing —which is also why
# `cacheDisplay` does not see them—, so what is checked is that it mounts as a
# real view and that the player does not fail. It is written down in
# docs/macos.md.
BUILD_LOG="$(mktemp)"
if cargo an macos examples/media --no-launch >"$BUILD_LOG" 2>&1; then
  echo "  ok   the map and video example builds"
else
  echo "  FAIL the map and video example does not build"
  tail -20 "$BUILD_LOG"
  fail=1
fi
rm -f "$BUILD_LOG"

SHOT_MEDIA="$ROOT/build/macos/media.png"
MEDIA_LOG="$(mktemp)"
AN_SCREENSHOT="$SHOT_MEDIA" AN_SCREENSHOT_FRAMES=240 \
  AN_SCREENSHOT_PRESS=360,782 "$BIN" >"$MEDIA_LOG" 2>&1 || true

if grep -qE "no se pinta en macOS|is not drawn on macOS" "$MEDIA_LOG"; then
  echo "  FAIL the map or the video still are not painted:"
  grep -E "no se pinta en macOS|is not drawn on macOS" "$MEDIA_LOG" | sed 's/^/       /'
  fail=1
else
  echo "  ok   the map and the video mount with their system view"
fi

if grep -qE "no se puede reproducir|cannot be played" "$MEDIA_LOG"; then
  echo "  FAIL the player failed:"
  grep -E "no se puede reproducir|cannot be played" "$MEDIA_LOG" | sed 's/^/       /'
  fail=1
else
  echo "  ok   the player did not fail on starting up"
fi

media_colours="$(sed -n 's/.*, \([0-9]*\) colours).*/\1/p' "$MEDIA_LOG" | tail -1)"
if [ -n "$media_colours" ] && [ "$media_colours" -gt 200 ]; then
  echo "  ok   the map really draws ($media_colours colours in the screenshot)"
else
  echo "  FAIL the map came out flat: ${media_colours:-no screenshot} colours"
  fail=1
fi

if [ "$fail" -ne 0 ]; then
  echo
  cat "$RUN_LOG" "$DESK_LOG" "$HOVER_LOG" "$MEDIA_LOG"
  rm -f "$RUN_LOG" "$DESK_LOG" "$HOVER_LOG" "$MEDIA_LOG"
  exit 1
fi
rm -f "$RUN_LOG" "$DESK_LOG" "$HOVER_LOG" "$MEDIA_LOG"
