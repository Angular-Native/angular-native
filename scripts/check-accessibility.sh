#!/usr/bin/env bash
# Accessibility on the Apple hosts: what a screen reader is actually told.
#
# **Setting a property does not prove a reader can read it.** That sentence is
# the whole reason this file is not four `grep`s. Every one of the six props of
# the contract can be written on the right object, with the right value, and
# still reach nobody: in AppKit a view that is not an accessibility element is
# never published, in UIKit traits on a view that is not an element reach no
# stop, and neither of those failures shows up in a log or in a screenshot. The
# host looks like it works and the app is mute.
#
# So the load-bearing check here is the outside one, and it exists on exactly
# one platform:
#
#   macOS — the app is launched, and a **separate process** reads its
#           accessibility tree through `AXUIElementCopyAttributeValue`, which is
#           the same door VoiceOver and Accessibility Inspector go through. The
#           walker never links against the app and cannot see one line of what
#           the host wrote; all it has is a pid. What comes back is what an
#           assistive client would get. See `scripts/ax-dump.swift`.
#
#           That walk found the bug that the rest of this file would have
#           missed: a role on its own left the view out of the tree entirely.
#
#   iOS, tvOS, visionOS — no outside route was found. The simulator does not
#           publish the guest app's tree to the host's accessibility API, and
#           there is no `simctl` verb that dumps it. What is checked instead is
#           what can be: that the six props travel with the right names, that
#           the trait vocabulary does not drift between the pieces that have to
#           agree, and that the host builds. Said plainly rather than dressed
#           up: on those three the reading is unverified from outside.
#
#   watchOS — same, with one thing more that can be checked: the whole
#           translation happens in Rust, and the names it emits have to exist in
#           the Swift table that turns them into `AccessibilityTraits`. Nothing
#           but this makes those two agree, and a name that drifts leaves a mute
#           view with no error anywhere.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0

check() { # <0 ok, 1 bad> <what was being checked>
  if [ "$1" -eq 0 ]; then
    echo "  ok   $2"
  else
    echo "  FALLO $2"
    fail=1
  fi
}

echo "== accessibility (Apple)"

# ---------------------------------------------------------------------------
# 1. The six props reach every Apple host.
#
# `check-wrapper.sh` cannot see these: it reads the `push({…})` of each
# directive, and the six are not in one — they are declared inputs that the
# contract landed without wiring. Until they are, this is what stands in for
# it, and it is worth having either way: it is the list of what has to be
# handled and where.
# ---------------------------------------------------------------------------
PROPS=(
  accessibilityLabel
  accessibilityHint
  accessibilityRole
  accessibilityValue
  accessibilityState
  accessible
)
for prop in "${PROPS[@]}"; do
  missing=()
  grep -q "\"$prop\"" crates/an-ios/src/accessibility.rs || missing+=("UIKit")
  grep -q "\"$prop\"" crates/an-macos/src/accessibility.rs || missing+=("AppKit")
  grep -q "\"$prop\"" crates/an-watch/src/snapshot.rs || missing+=("watchOS")
  [ "${#missing[@]}" -eq 0 ] && r=0 || r=1
  check $r "[$prop] is looked at by every Apple host"
  if [ "${#missing[@]}" -ne 0 ]; then
    echo "       nobody looks at it in: ${missing[*]}"
  fi
done

# ---------------------------------------------------------------------------
# 2. The twelve roles of the contract are decided, one by one, in all three.
#
# The Rust side is already safe by construction —the three tables are exhaustive
# `match`es over `Role`, so adding a role to the contract stops the build— but
# only if the vocabulary in `an-core` is the same one the contract declares.
# Nothing links the TypeScript union to the Rust enum, so that is the join that
# is checked here.
# ---------------------------------------------------------------------------
CONTRACT="$(awk '/^export type NativeRole =/,/^$/' packages/primitives/src/primitives.ts \
  | grep -oE "'[a-z]+'" | tr -d "'" | sort -u)"
CORE="$(awk '/pub fn parse\(raw: &str\)/,/^    \}/' crates/an-core/src/accessibility.rs \
  | grep -oE '^            "[a-z]+" =>' | grep -oE '"[a-z]+"' | tr -d '"' | sort -u)"
diff <(echo "$CONTRACT") <(echo "$CORE") >/dev/null 2>&1 && r=0 || r=1
check $r "the core knows exactly the roles the contract declares"
if [ "$r" -ne 0 ]; then
  echo "       only in the contract: $(comm -23 <(echo "$CONTRACT") <(echo "$CORE") | tr '\n' ' ')"
  echo "       only in the core:     $(comm -13 <(echo "$CONTRACT") <(echo "$CORE") | tr '\n' ' ')"
fi

# The states, the same way. `mixed` is not a key, it is a value of `checked`,
# so the list is the five field names of `NativeAccessibilityState`.
CONTRACT_STATE="$(awk '/^export interface NativeAccessibilityState \{/,/^\}/' \
  packages/primitives/src/primitives.ts \
  | grep -oE '^  [a-z]+\?:' | tr -d ' ?:' | sort -u)"
CORE_STATE="$(awk '/^pub struct State \{/,/^\}/' crates/an-core/src/accessibility.rs \
  | grep -oE '^    pub [a-z]+:' | sed 's/pub //;s/[ :]//g' | sort -u)"
diff <(echo "$CONTRACT_STATE") <(echo "$CORE_STATE") >/dev/null 2>&1 && r=0 || r=1
check $r "and exactly the states it declares"

# ---------------------------------------------------------------------------
# 3. watchOS: the two halves of the trait table.
#
# The watch is the only host split across two languages here. Rust decides which
# `AccessibilityTraits` a role becomes and sends the **name**; Swift turns the
# name into the constant. A name Rust emits and Swift does not know is a trait
# that silently never gets applied — the shell says so at run time, but only if
# somebody is watching the log of a watch simulator, which nobody is.
# ---------------------------------------------------------------------------
EMITTED="$( { awk '/^fn swiftui_trait/,/^\}/' crates/an-watch/src/snapshot.rs
  awk '/fn accessibility_traits_of/,/^\}/' crates/an-watch/src/snapshot.rs; } \
  | grep -oE '"is[A-Za-z]+"' | tr -d '"' | sort -u)"
KNOWN="$(awk '/private static func trait/,/^    \}/' shells/watchos/Sources/AnAccessibility.swift \
  | grep -oE 'case "is[A-Za-z]+"' | grep -oE '"is[A-Za-z]+"' | tr -d '"' | sort -u)"
UNKNOWN="$(comm -23 <(echo "$EMITTED") <(echo "$KNOWN"))"
[ -z "$UNKNOWN" ] && r=0 || r=1
check $r "every trait Rust sends to the watch exists in the shell's table"
if [ -n "$UNKNOWN" ]; then
  echo "       Rust sends and Swift does not know: $(echo "$UNKNOWN" | tr '\n' ' ')"
fi

# And the reverse, which is not a bug but is dead weight: a constant the shell
# translates and Rust never sends.
DEAD="$(comm -13 <(echo "$EMITTED") <(echo "$KNOWN"))"
[ -z "$DEAD" ] && r=0 || r=1
check $r "and the shell translates nothing Rust never sends"
if [ -n "$DEAD" ]; then
  echo "       in the shell and unreachable: $(echo "$DEAD" | tr '\n' ' ')"
fi

# ---------------------------------------------------------------------------
# 4. The watch shell compiles.
#
# It is not built by anything else that runs without a device: `check-watchos.sh`
# cross-compiles the Rust crate and stops there, so a Swift error in the shell
# would only show up when somebody ran `cargo an watchos`. Type-checking is
# enough and needs no linking, so it needs neither the staticlib nor nightly.
# ---------------------------------------------------------------------------
if [ "$(uname -s)" != "Darwin" ]; then
  echo "  --   the rest is skipped: the Apple hosts only build on a Mac"
  exit "$fail"
fi

if SDK="$(xcrun --sdk watchsimulator --show-sdk-path 2>/dev/null)" && [ -n "$SDK" ]; then
  if xcrun swiftc -typecheck -sdk "$SDK" \
      -target arm64-apple-watchos11.0-simulator \
      -parse-as-library \
      -import-objc-header shells/watchos/Sources/Bridging-Header.h \
      -I crates/an-watch/include \
      shells/watchos/Sources/*.swift shells/shared/*.swift 2>/dev/null; then
    echo "  ok   the watch shell type-checks, accessibility modifiers included"
  else
    echo "  FALLO the watch shell does not type-check"
    xcrun swiftc -typecheck -sdk "$SDK" \
      -target arm64-apple-watchos11.0-simulator \
      -parse-as-library \
      -import-objc-header shells/watchos/Sources/Bridging-Header.h \
      -I crates/an-watch/include \
      shells/watchos/Sources/*.swift shells/shared/*.swift 2>&1 | head -20
    fail=1
  fi
else
  echo "  --   the watch shell is not type-checked: no watchsimulator SDK"
fi

# ---------------------------------------------------------------------------
# 5. And the one that counts: the tree as the system publishes it.
#
# Everything above compares text with text. This launches the app and asks the
# accessibility server, from another process, what that app publishes. It needs
# the Accessibility permission, which is granted by hand per app in System
# Settings and cannot be granted from a script — so when it is missing this says
# so and skips. A machine without the grant has not broken anything; it just
# cannot see this.
# ---------------------------------------------------------------------------
AX_DUMP="$ROOT/build/macos/ax-dump"
mkdir -p "$ROOT/build/macos"
if ! swiftc -O "$ROOT/scripts/ax-dump.swift" -o "$AX_DUMP" 2>/dev/null; then
  echo "  FALLO the accessibility walker does not compile"
  swiftc -O "$ROOT/scripts/ax-dump.swift" -o "$AX_DUMP" 2>&1 | head -20
  exit 1
fi

# The walker itself says whether it is allowed to look. Pid 1 publishes no tree,
# so a run against it separates "no permission" (2) from anything else.
"$AX_DUMP" 1 >/dev/null 2>&1 || permission=$?
if [ "${permission:-0}" -eq 2 ]; then
  echo "  --   the accessibility tree is not read: this process has no Accessibility"
  echo "       permission. System Settings > Privacy & Security > Accessibility,"
  echo "       granted to the terminal running this."
  exit "$fail"
fi

BUILD_LOG="$(mktemp)"
if cargo an macos examples/a11y-apple --no-launch >"$BUILD_LOG" 2>&1; then
  echo "  ok   the accessibility example builds into the .app"
else
  echo "  FALLO the accessibility example does not build"
  tail -30 "$BUILD_LOG"
  rm -f "$BUILD_LOG"
  exit 1
fi
rm -f "$BUILD_LOG"

BIN="$ROOT/build/macos/AngularNativeMac.app/Contents/MacOS/AngularNativeMac"
RUN_LOG="$(mktemp)"
TREE="$ROOT/build/macos/accessibility-tree.txt"

# A copy left over from an earlier run poisons the reading: two processes of the
# same app confuse the accessibility server, and what comes back is a tree with
# no window at all. It cost an afternoon to find, so it is swept before and
# after rather than trusted.
pkill -9 -f "AngularNativeMac.app/Contents/MacOS/AngularNativeMac" 2>/dev/null || true

"$BIN" >"$RUN_LOG" 2>&1 &
APP_PID=$!
# Out of the shell's job table: without this, killing it at the end prints a
# "Killed: 9" of its own on top of the report.
disown "$APP_PID" 2>/dev/null || true
trap 'kill -9 "$APP_PID" 2>/dev/null || true' EXIT

# The window has to exist before there is anything to read. Polling beats a
# fixed sleep: on a busy machine the first frame can take a while, and a check
# that fails because the laptop was compiling something else is a check nobody
# believes.
for _ in $(seq 1 60); do
  sleep 0.5
  if "$AX_DUMP" "$APP_PID" >"$TREE" 2>/dev/null && grep -q AXWindow "$TREE"; then
    break
  fi
done

if grep -q AXWindow "$TREE" 2>/dev/null; then
  echo "  ok   the app publishes an accessibility tree to a process outside it"
else
  echo "  FALLO nothing came back from the accessibility tree"
  tail -20 "$RUN_LOG"
  exit 1
fi

# Only the window: the menu bar is the system's, and it is four thousand lines.
WINDOW="$(sed -n '/AXWindow/,/AXMenuBar/p' "$TREE")"

# What has to be in there. Each line is one decision of the host, and it is
# written as the reader would receive it — role, name, value — not as the
# property that was set.
expect() { # <regexp> <what it proves>
  grep -qE -- "$1" <<<"$WINDOW" && r=0 || r=1
  check $r "$2"
}

expect 'AXHeading value=" Accessibility "' \
  'role="header" comes back as AXHeading'
expect 'AXButton label="Play" help="Starts the track' \
  'the label and the hint of a plain view reach the reader'
expect 'AXCheckBox\[AXSwitch\] label="Night mode" value="1"' \
  'role="switch" is AXCheckBox with the AXSwitch subrole, and checked is its value'
expect 'AXSlider label="Volume" value="60 per cent"' \
  'role="slider" is AXSlider and the value is the one the template wrote'
expect 'AXButton label="Chosen and switched off" enabled=false selected=true' \
  'disabled and selected come back as the two properties AppKit has for them'
expect 'AXRadioButton label="No trait in UIKit"' \
  'role="radio", which UIKit has not, is AXRadioButton here'

expect 'AXUnknown title="Stripped of its role"' \
  'role="none" takes the role away and leaves the name, like android.view.View'
expect 'AXUnknown label="Named and nothing else"' \
  'a name on a plain view is enough to make it a stop'

# The two that guard against the easy mistake: writing our label over the
# system's. An `NSButton` arrives with its title as its name and with a role
# AppKit works out for itself, and both are lost the moment anything is
# overridden on the view. The second row is the one that catches it.
expect 'AXButton title="Untouched button"' \
  'a system button nobody labelled keeps the name AppKit gave it'
expect 'AXButton label="Save the changes you made" title="Save"' \
  'and one the template did label reads with ours and is still a button'

# And what must **not** be there. An accessibility check that only looks for
# what it expects cannot catch the opposite failure: something read out that
# should be silent.
absent() { # <regexp> <what it proves>
  grep -qE -- "$1" <<<"$WINDOW" && r=1 || r=0
  check $r "$2"
}

absent 'decorative filler' \
  'accessible="false" takes the text inside out of the tree with it'
absent 'AXStaticText value="Play"' \
  'accessible="true" makes the row one stop: its icon and its text are not two more'

# The log side of the same coin: what could not be applied has to have been
# said. A host that drops something quietly passes every check above.
said() { # <regexp> <what it proves>
  grep -qE -- "$1" "$RUN_LOG" && r=0 || r=1
  check $r "$2"
}

expect 'AXUnknown label="No role in AppKit"' \
  'role="summary", which AppKit has not, keeps the name and claims no role'
said 'accessibilityRole.="summary". on <View>: AppKit has no role' \
  'and it says so, with the name of what was asked for'
said 'asks for .busy.: the NSAccessibility protocol has no property' \
  'busy, which no Apple platform has, is said too'

kill -9 "$APP_PID" 2>/dev/null || true
trap - EXIT
rm -f "$RUN_LOG"

echo "       the tree that was read is in build/macos/accessibility-tree.txt"

exit "$fail"
