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
    echo "  FAIL  $2"
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
    echo "  FAIL  the watch shell does not type-check"
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
# accessibility server, from another process, what that app publishes.
#
# **It needs the Accessibility permission, and that permission is granted by
# hand.** A checkbox in System Settings, per application, which no script can
# tick — so on a machine where nobody ticked it, this half cannot run. That
# includes every CI runner there will ever be, and it includes a laptop that has
# just been set up. A machine without the grant has not broken anything, and a
# suite that goes red on it is lying about the state of the code: the whole of
# `scripts/` is written so that what cannot be checked is *skipped*, out loud
# and with its reason, the way `check-signing.sh` skips the `.aab` when
# bundletool is not installed.
#
# Three states have to be told apart, and only one of them is a bug:
#
#   the permission is missing         -> skip, saying what to grant and where
#   the permission works, app is mute -> FAIL: that is the regression this
#                                        whole file exists to catch
#   the permission works, app talks   -> carry on
#
# Telling the first from the second is not one question. `AXIsProcessTrusted()`,
# which is what the walker checks on the way in and what makes it exit 2, reads
# TCC's record — and that record can say yes while the server refuses every
# read. A terminal that was granted and then replaced by an update is the usual
# way in; over ssh there is no grant to have. In that state the trust check
# passes and every `AXUIElementCopyAttributeValue` comes back empty, which looks
# exactly like a host that publishes nothing.
#
# So the permission is not only asked about, it is *used*. A process that
# certainly publishes a tree is read first. If that one answers, the door works
# and a silent app is our bug; if it does not, the door is shut and there is
# nothing here to see.
# ---------------------------------------------------------------------------
AX_DUMP="$ROOT/build/macos/ax-dump"
mkdir -p "$ROOT/build/macos"
if ! swiftc -O "$ROOT/scripts/ax-dump.swift" -o "$AX_DUMP" 2>/dev/null; then
  echo "  FAIL  the accessibility walker does not compile"
  swiftc -O "$ROOT/scripts/ax-dump.swift" -o "$AX_DUMP" 2>&1 | head -20
  exit 1
fi

no_permission() { # <why>
  echo "  --   the accessibility tree is not read: $1."
  echo "       Grant Accessibility to the terminal running this — System Settings >"
  echo "       Privacy & Security > Accessibility — and run it again. Everything"
  echo "       above this line was checked; nothing below it can be."
  exit "$fail"
}

# The walker's own answer. Pid 1 publishes no tree, so a run against it can only
# come back as "no permission" (2) or as "nothing there" (1), and the first is
# the one worth acting on.
"$AX_DUMP" 1 >/dev/null 2>&1 || trusted=$?
if [ "${trusted:-0}" -eq 2 ]; then
  no_permission "this process is not trusted for Accessibility"
fi

# And the door, tried rather than asked about.
#
# The Finder is read: it is always running, and it is a *regular* app, which is
# the distinction that matters. The Dock and the menu bar extras publish
# themselves to anybody — that is how the menu bar is usable — so reading one of
# those proves nothing. Reading another regular application's windows is what
# the permission actually governs.
#
# What a refusal looks like from here is the thing that cost the time. There is
# **no error**: `AXUIElementCopyAttributeValue` returns `kAXErrorSuccess`, hands
# back an array of the right length, and every element in it is the application
# element again — the app as its own child, all the way down until the walker's
# depth limit stops it. So the tree is not empty, it is a spiral, and a check
# that only asks "did anything come back?" says yes and then fails to find a
# single window in it. That is what "nothing came back from the accessibility
# tree" was really reporting, and it is not a bug in this repository.
#
# The signature is exactly that: the first child of the application is the
# application. One line of the dump settles it, so the walker is killed as soon
# as it has written two — the spiral is thousands of rows and none of them are
# any more informative than the second.
WITNESS_OUT="$(mktemp)"
WITNESS_PID="$(pgrep -x Finder | head -1 || true)"
witness=unknown
if [ -n "$WITNESS_PID" ]; then
  "$AX_DUMP" "$WITNESS_PID" >"$WITNESS_OUT" 2>/dev/null &
  WALKER=$!
  for _ in $(seq 1 40); do
    [ "$(wc -l <"$WITNESS_OUT")" -ge 2 ] && break
    # It stopped on its own before saying anything: that is a refusal too, and
    # waiting the other three seconds out would prove nothing.
    kill -0 "$WALKER" 2>/dev/null || break
    sleep 0.1
  done
  # Killed before it is waited on, always: the walk does not end in four
  # seconds and `wait` on a live walker never comes back.
  kill -9 "$WALKER" 2>/dev/null || true
  wait "$WALKER" 2>/dev/null || true

  if [ "$(wc -l <"$WITNESS_OUT")" -lt 2 ]; then
    witness=silent
  elif [ "$(sed -n '1p;2p' "$WITNESS_OUT" | grep -c '^ *AXApplication')" -eq 2 ]; then
    witness=spiral
  else
    witness=answers
  fi
fi
rm -f "$WITNESS_OUT"

case "$witness" in
  silent)
    no_permission "the Finder publishes nothing to this process"
    ;;
  spiral)
    # TCC's record said yes and the server still said no. It is the same wall
    # as an unticked box, reached from the other side, and the same instruction
    # gets over it: granting it again replaces the stale record.
    no_permission "the Finder comes back as its own child, which is what a refused read is"
    ;;
  unknown)
    echo "  --   the accessibility tree is not read: there is no Finder running, so this"
    echo "       is not a Mac with a logged-in session and there is nothing for an"
    echo "       app to publish a tree to."
    exit "$fail"
    ;;
esac

# Every copy of this `.app` that is running, by pid.
#
# `pgrep -c` is a Linux flag and BSD pgrep has no counter, so what comes back is
# the list and the caller counts it. Without the `|| true` a pgrep that matches
# nothing exits 1 and takes the script with it under `set -e`.
copies() {
  pgrep -f "AngularNativeMac.app/Contents/MacOS/AngularNativeMac" 2>/dev/null || true
}

# Two copies of the same app confuse the accessibility server: what comes back
# is a tree with no window at all, which reads exactly like a host that
# publishes nothing. It cost an afternoon to find.
#
# This used to be a `pkill -9` of everything that matched, which is worse than
# the flake it was papering over: `check-macos.sh`, a screenshot run and this
# script all drive the *same* binary, so the sweep silently killed whatever
# somebody else was in the middle of and then read a tree it had no reason to
# trust. There is no way from here to tell a leftover of an earlier run from a
# live one belonging to another terminal, so neither is killed and neither is
# guessed at: if anything is already running, this refuses, names the pids and
# says what clears them. A refusal that names the reason is worth more than a
# reading that may be of the wrong process.
BEFORE="$(copies)"
if [ -n "$BEFORE" ]; then
  echo "  --   the accessibility tree is not read: the app is already running as pid(s)"
  echo "       $(echo "$BEFORE" | tr '\n' ' ')and two copies of one application answer the"
  echo "       accessibility server with no window at all. Something else is driving this"
  echo "       .app: a screenshot, check-macos.sh, or a run of this that was interrupted."
  echo "       Clear it with \`pkill -f AngularNativeMac.app/Contents/MacOS/AngularNativeMac\`"
  echo "       and run this again. Everything above this line was checked; nothing below"
  echo "       it can be."
  exit "$fail"
fi

BUILD_LOG="$(mktemp)"
if cargo an macos examples/a11y --no-launch >"$BUILD_LOG" 2>&1; then
  echo "  ok   the accessibility example builds into the .app"
else
  echo "  FAIL  the accessibility example does not build"
  tail -30 "$BUILD_LOG"
  rm -f "$BUILD_LOG"
  exit 1
fi
rm -f "$BUILD_LOG"

BIN="$ROOT/build/macos/AngularNativeMac.app/Contents/MacOS/AngularNativeMac"
RUN_LOG="$(mktemp)"
TREE="$ROOT/build/macos/accessibility-tree.txt"

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
# The window shows up before anything is mounted in it, so waiting for the
# window is waiting for the wrong thing: what has to be there is something
# *inside* it. Waiting for the window alone reads an empty tree and reports
# every assertion as a failure, which looks exactly like a broken host.
window_rows() {
  sed -n "/AXWindow/,/AXMenuBar/p" "$TREE" 2>/dev/null | wc -l
}

# A copy that turns up *after* the launch poisons the reading exactly as a
# leftover one does, and it is not hypothetical: `check-macos.sh`, any
# screenshot run and this script all drive the same binary. So the identity is
# not established once and then trusted — it is asserted on every turn of the
# loop, and the refusal comes out the moment a second copy appears instead of
# thirty seconds later, after polling a tree that was never going to fill.
not_only_ours() { # <the pids that are running>
  kill -9 "$APP_PID" 2>/dev/null || true
  trap - EXIT
  echo "  --   the accessibility tree is not read: this launched pid $APP_PID and what is"
  echo "       running is $(echo "$1" | tr '\n' ' ')— with two copies up the accessibility server"
  echo "       answers with no window at all, so there is no telling whose tree came back."
  echo "       Something else is driving this .app: a screenshot, check-macos.sh."
  echo "       Everything above this line was checked; nothing below it can be."
  exit "$fail"
}

# The walker's exit code is read and not thrown away. `|| continue` would
# swallow a 2 — the permission going away between the check above and here, a
# grant revoked while this ran — and sixty silent retries later this would
# report the host as mute, which is the wrong bug entirely.
for _ in $(seq 1 60); do
  sleep 0.5

  running="$(copies)"
  if [ -z "$running" ]; then
    # Nothing is running at all, so the app died on its own: that is a real
    # failure and it is not the accessibility server's. Its log is the only
    # place the reason will be.
    trap - EXIT
    echo "  FAIL  the app exited before it published anything"
    tail -20 "$RUN_LOG"
    exit 1
  fi
  [ "$running" = "$APP_PID" ] || not_only_ours "$running"

  walked=0
  "$AX_DUMP" "$APP_PID" >"$TREE" 2>/dev/null || walked=$?
  if [ "$walked" -eq 2 ]; then
    kill -9 "$APP_PID" 2>/dev/null || true
    trap - EXIT
    no_permission "the Accessibility permission went away while this was running"
  fi
  if [ "$(window_rows)" -gt 8 ]; then
    break
  fi
done

if [ "$(window_rows)" -gt 8 ]; then
  echo "  ok   the app publishes an accessibility tree to a process outside it"
else
  # And this is a real failure, said as one. The permission is not the reason
  # and does not get to be suspected: the Dock's tree was read from this very
  # process a moment ago, so the door works and it is our app that is mute.
  echo "  FAIL  nothing came back from the accessibility tree, and the permission is"
  echo "        not why: the Finder published a real one to this process seconds ago"
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

expect 'AXHeading label="Settings"' \
  'role="header" comes back as AXHeading'
expect 'AXButton label="Save the draft" help="saves it' \
  'the label and the hint of a plain view reach the reader'
expect 'AXCheckBox\[AXSwitch\] label="Night mode" value="1"' \
  'role="switch" is AXCheckBox with the AXSwitch subrole, and checked is its value'
expect 'AXCheckBox label="Select all" value="2"' \
  'checked="mixed", which UIKit cannot say, is a value of its own here'
expect 'AXSlider label="Loudness" value="60 per cent"' \
  'role="slider" is AXSlider and the value is the one the template wrote'
expect 'label="Volume" .*enabled=false selected=true' \
  'disabled and selected come back as the two properties AppKit has for them'
expect 'AXRadioButton label="An option"' \
  'role="radio", which UIKit has not, is AXRadioButton here'
expect 'AXUnknown title="Stripped of its role"' \
  'role="none" takes the role away and leaves the name, like android.view.View'
expect 'label="The name beats the testID"' \
  'a name on a plain view is enough to make it a stop'

# The two that guard against the easy mistake: writing our label over the
# system's. An `NSButton` arrives with its title as its name and with a role
# AppKit works out for itself, and both are lost the moment anything is
# overridden on the view. The second row is the one that catches it.
expect 'AXButton title="OK"' \
  'a system button nobody labelled keeps the name AppKit gave it'
expect 'AXButton label="Save the changes you made" title="Save"' \
  'and one the template did label reads with ours and is still a button'

# And the pair that proves the role written back is the one the control really
# has, not the one its class suggests. `an-activity-indicator` and
# `an-progress-bar` are both an `NSProgressIndicator`; AppKit separates them by
# style, and naming them is exactly what makes the host supply the role. A
# spinner coming back as `AXProgressIndicator` is a reader being told about
# progress that does not exist, and nothing but this line can see it.
expect 'AXBusyIndicator label="Still working"' \
  'a named spinner is still a spinner, not the progress bar it shares a class with'
expect 'AXProgressIndicator label="Half done"' \
  'and the bar of the same class keeps the role that is genuinely its own'

# And what must **not** be there. An accessibility check that only looks for
# what it expects cannot catch the opposite failure: something read out that
# should be silent.
absent() { # <regexp> <what it proves>
  grep -qE -- "$1" <<<"$WINDOW" && r=1 || r=0
  check $r "$2"
}

absent 'DECORATION NOBODY READS' \
  'accessible="false" takes the text inside out of the tree with it'
absent 'value="Three unread messages"' \
  'accessible="true" makes the row one stop: what is inside is not more of them'

# The log side of the same coin: what could not be applied has to have been
# said. A host that drops something quietly passes every check above.
said() { # <regexp> <what it proves>
  grep -qE -- "$1" "$RUN_LOG" && r=0 || r=1
  check $r "$2"
}

expect 'AXUnknown label="A summary"' \
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
