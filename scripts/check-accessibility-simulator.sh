#!/usr/bin/env bash
# UIKit accessibility, read from outside the app — through the simulator.
#
# This is the other half of `check-accessibility.sh`, and it is separate for the
# same reason the Android side splits `check-a11y-device.sh` out: it needs
# something the machine may not have, it opens a window, and it takes a couple
# of minutes. `check-all.sh` runs the cheap half; this one is run on purpose.
#
# **Why it is possible at all.** The iOS Simulator bridges the guest app's
# accessibility tree into the *host's* macOS accessibility API — that is how
# Accessibility Inspector inspects a simulator — so the same walker that reads
# a Mac app, `scripts/ax-dump.swift`, can read an iPhone app by asking
# `Simulator.app` instead. What comes back is UIKit's tree translated into AX
# attributes: `.header` arrives as `AXHeading`, `.toggleButton` as `AXCheckBox`
# with the `AXSwitch` subrole, `.adjustable` as `AXSlider`.
#
# It is not the same as VoiceOver speaking, and it is not claimed to be. What it
# is, is the tree an assistive client sees, read by a process that never linked
# against the app — which is the thing that cannot be faked by writing a
# property and reading it back.
#
# It needs the Accessibility permission, granted by hand in System Settings to
# whatever runs this. Without it, this says so and skips.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0

check() { # <0 ok, 1 bad> <what was being checked>
  if [ "$1" -eq 0 ]; then
    echo "  ok   $2"
  else
    echo "  FAIL  $2"
    fail=1
  fi
}

echo "== accessibility in the iOS simulator"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "  --   skipped: the simulator only exists on a Mac"
  exit 0
fi

if ! xcrun simctl list devices available >/dev/null 2>&1; then
  echo "  --   skipped: no usable simulator (xcrun simctl says nothing is available)"
  exit 0
fi

AX_DUMP="$ROOT/build/macos/ax-dump"
mkdir -p "$ROOT/build/macos"
if ! swiftc -O "$ROOT/scripts/ax-dump.swift" -o "$AX_DUMP" 2>/dev/null; then
  echo "  FAIL  the accessibility walker does not compile"
  swiftc -O "$ROOT/scripts/ax-dump.swift" -o "$AX_DUMP" 2>&1 | head -20
  exit 1
fi

"$AX_DUMP" 1 >/dev/null 2>&1 || permission=$?
if [ "${permission:-0}" -eq 2 ]; then
  echo "  --   skipped: this process has no Accessibility permission, so it cannot read"
  echo "       another app's tree. System Settings > Privacy & Security > Accessibility."
  exit 0
fi

BUILD_LOG="$(mktemp)"
if cargo an ios examples/a11y >"$BUILD_LOG" 2>&1; then
  echo "  ok   the example builds, installs and launches in the simulator"
else
  echo "  FAIL  the example did not get as far as running"
  tail -30 "$BUILD_LOG"
  rm -f "$BUILD_LOG"
  exit 1
fi
rm -f "$BUILD_LOG"

SIM_PID="$(pgrep -f 'Simulator.app/Contents/MacOS/Simulator' | head -1 || true)"
if [ -z "$SIM_PID" ]; then
  echo "  FAIL  Simulator.app is not running, so there is no tree to read"
  exit 1
fi

TREE="$ROOT/build/macos/accessibility-tree-ios.txt"

# The guest app's own tree hangs off a group the simulator publishes for it.
# Everything above that is the simulator's chrome — its side buttons, its
# toolbar — and everything after is the Mac's menu bar.
# The group shows up before the app has anything in it, so waiting for the
# group is waiting for the wrong thing: what has to be there is something
# *inside* it.
for _ in $(seq 1 60); do
  sleep 0.5
  "$AX_DUMP" "$SIM_PID" >"$TREE" 2>/dev/null || continue
  if [ "$(sed -n '/iOSContentGroup/,/AXToolbar/p' "$TREE" | wc -l)" -gt 3 ]; then
    break
  fi
done

if [ "$(sed -n '/iOSContentGroup/,/AXToolbar/p' "$TREE" | wc -l)" -gt 3 ]; then
  echo "  ok   the simulator publishes the app's tree to a process outside it"
else
  echo "  FAIL  nothing came back from the simulator's accessibility tree"
  exit 1
fi

SCREEN="$(sed -n '/iOSContentGroup/,/AXToolbar/p' "$TREE")"

expect() { # <regexp> <what it proves>
  grep -qE -- "$1" <<<"$SCREEN" && r=0 || r=1
  check $r "$2"
}

absent() { # <regexp> <what it proves>
  grep -qE -- "$1" <<<"$SCREEN" && r=1 || r=0
  check $r "$2"
}

# The traits, as they reach a client. UIKit's mask is not published as a mask:
# the bridge turns it into a role, which is what makes this readable at all —
# and what makes it worth checking, because the mapping from `.header` to
# `AXHeading` is the platform's, not ours.
expect 'AXHeading label="Settings"' \
  'the .header trait arrives as AXHeading'
expect 'AXButton label="Save the draft" help="saves it' \
  '.button with the label and the hint of a plain view'
expect 'AXCheckBox\[AXSwitch\] label="Night mode" value="1"' \
  '.toggleButton is what UIKit has for a switch, and checked is its value'
expect 'AXSlider label="Loudness" value="60 per cent"' \
  '.adjustable arrives as AXSlider, with the value the template wrote'
expect 'label="Volume" .*enabled=false selected=true' \
  '.notEnabled and .selected are the two bits of state UIKit keeps in the mask'
expect 'label="A summary"' \
  'the row AppKit has no role for is a stop here, where the trait does exist'
expect 'AXButton label="Save the changes you made"' \
  'a labelled system button reads with our name and is still a button'
expect 'label="The name beats the testID"' \
  'a name on a plain view is enough to make it a stop here too'

# The one UIKit has no answer for. It is not published as a radio button
# because there is no such trait — and the app said so in its log.
absent 'AXRadioButton' \
  'role="radio", which UIKit has not, is not faked as anything else'

# And the grouping, which is the same idea as on the Mac and a different API:
# `isAccessibilityElement` on the row, not an emptied children list.
absent 'DECORATION NOBODY READS' \
  'accessible="false" takes the text inside out of the tree with it'
absent 'label="Three unread messages"' \
  'the grouped row is one stop, not a container with more inside'

echo "       the tree that was read is in build/macos/accessibility-tree-ios.txt"

exit "$fail"
