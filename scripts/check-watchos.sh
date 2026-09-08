#!/usr/bin/env bash
# The watch host: that it cross-compiles, that the model SwiftUI sees is the one
# the layout computed, and that the three lists that have to say the same thing
# do say it.
#
# The cross-compilation is kept apart from the rest because it needs nightly:
# `aarch64-apple-watchos-sim` is a tier 3 target and does not ship a precompiled
# `std`, so it has to be built on the spot with `-Z build-std`. If nightly is
# missing, this reports it and does not fail: the rest of the checks have no
# reason to fall over because somebody is short a toolchain.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0

echo "== watchOS"

# The result is passed in already evaluated and with `&& r=0 || r=1` in front,
# not as a bare `$?`: `set -e` kills the script as soon as a check returns 1, and
# what is wanted is that it reports them all and fails at the end.
check() { # <0 if good, 1 if bad> <what was being checked>
  if [ "$1" -eq 0 ]; then
    echo "  ok   $2"
  else
    echo "  FAIL $2"
    fail=1
  fi
}

# 1. The model. It runs on the Mac and needs neither a simulator nor nightly: the
#    host applies MountOp to a data structure, and that is what is checked here.
# The output is kept rather than thrown away. A `>/dev/null 2>&1` here
# cannot tell a test that failed from a build that did, and this line has
# already cost two investigations of a failure that reproduces nowhere
# else: what a check hides is what somebody pays for later.
CARGO_LOG="$(mktemp)"
if cargo test --quiet -p an-watch >"$CARGO_LOG" 2>&1; then
  echo "  ok   the model SwiftUI sees is built as it should be"
else
  echo "  FAIL the an-watch tests do not pass"
  # What the run that failed said, not what a fresh one says. Re-running was
  # hiding the cause: the second attempt passes, so the tail printed a wall of
  # green under the word FAIL and the real reason was never seen.
  tail -30 "$CARGO_LOG"
  fail=1
fi

# 2. The wire between the two languages.
#
#    The snapshot is a JSON object serialised from `snapshot::Node` and decoded
#    into Swift's `AnNode` with `.convertFromSnakeCase`, so the two field lists
#    are one list written twice. Nothing makes them agree: a field Rust stops
#    sending decodes to nil and the prop goes quiet, and a field Swift never
#    declares is dropped by `JSONDecoder` without a word. `clip` was sent on
#    every frame and read by nobody, which left the shell clipping
#    unconditionally while `overflow: visible` worked on the other three hosts.
snake_to_camel() {
  awk -F_ '{ s = $1; for (i = 2; i <= NF; i++) s = s toupper(substr($i, 1, 1)) substr($i, 2); print s }'
}
for pair in "Node:AnNode" "Snapshot:AnSnapshot"; do
  RS="${pair%%:*}"; SW="${pair##*:}"
  RUST="$(awk "/^pub struct $RS \\{/,/^\\}/" crates/an-watch/src/snapshot.rs \
    | sed -n 's/^    pub \([a-z_0-9]*\):.*/\1/p' | snake_to_camel | sort -u)"
  SWIFT="$(awk "/^struct $SW/,/^\\}/" shells/watchos/Sources/AnTree.swift \
    | sed -n 's/^    let \([a-zA-Z0-9]*\):.*/\1/p' | sort -u)"
  if [ -z "$RUST" ] || [ -z "$SWIFT" ]; then
    echo "  FAIL could not read the fields of $RS or $SW; the check is looking at the wrong shape"
    fail=1
    continue
  fi
  UNREAD="$(comm -23 <(echo "$RUST") <(echo "$SWIFT") | tr '\n' ' ')"
  UNSENT="$(comm -13 <(echo "$RUST") <(echo "$SWIFT") | tr '\n' ' ')"
  [ -z "$UNREAD" ] && r=0 || r=1
  check $r "every field $RS serialises is one $SW decodes${UNREAD:+ (nobody reads: $UNREAD)}"
  [ -z "$UNSENT" ] && r=0 || r=1
  check $r "every field $SW decodes is one $RS sends${UNSENT:+ (never sent: $UNSENT)}"
done

# 3. The three lists of primitives.
#
#    The vocabulary is in `an-core`, what the watch does not paint is in
#    `an-watch/src/snapshot.rs`, and what it does paint is in Swift. Nothing
#    forces the three to say the same thing except this: without it, adding a
#    primitive to the core would leave it on the watch as a hole nobody decided
#    on.
ALL="$(grep -oE '=> NodeKind::[A-Za-z]+' crates/an-core/src/props.rs \
  | sed 's/.*NodeKind:://' | sort -u)"
UNPAINTED="$(awk '/^pub fn unsupported/,/^\}/' crates/an-watch/src/snapshot.rs \
  | grep -oE 'NodeKind::[A-Za-z]+' | sed 's/NodeKind:://' | sort -u)"
# What the shell draws: the `case` arms of the `switch` in `AnNodeView`, plus
# `View`, which is the default branch, plus the two the system presents and that
# therefore live in `AnOverlays`.
PAINTED="$( { awk '/private var content: some View/,/^    \}/' shells/watchos/Sources/AnNodeView.swift \
    | sed -n 's/.*case "\([A-Za-z]*\)".*/\1/p'
  sed -n 's/.*case "\([A-Za-z]*\)".*/\1/p' shells/watchos/Sources/AnOverlays.swift
  echo View; } | sort -u)"

OVERLAP="$(comm -12 <(echo "$UNPAINTED") <(echo "$PAINTED"))"
[ -z "$OVERLAP" ] && r=0 || r=1
check $r "no primitive is both painted and discarded"
if [ -n "$OVERLAP" ]; then echo "       in both lists: $(echo "$OVERLAP" | tr '\n' ' ')"; fi

UNDECIDED="$(comm -23 <(echo "$ALL") <(cat <(echo "$UNPAINTED") <(echo "$PAINTED") | sort -u))"
[ -z "$UNDECIDED" ] && r=0 || r=1
check $r "every core primitive is either painted by the watch or says why not"
if [ -n "$UNDECIDED" ]; then echo "       undecided: $(echo "$UNDECIDED" | tr '\n' ' ')"; fi

# 4. The gestures. What the shell hooks up and what the host says never arrives
#    cannot overlap: a gesture that is hooked up and on top of that warns that it
#    does not work is worse than either of the two on its own.
HOOKED="$(grep -oE 'listens\(to: "[a-zA-Z]+"\)' shells/watchos/Sources/*.swift \
  | sed 's/.*"\(.*\)".*/\1/' | sort -u)"
NEVER_ARRIVE="$(awk '/^fn unheard/,/^\}/' crates/an-watch/src/snapshot.rs \
  | grep -oE '^\s+"[a-zA-Z]+"( \| "[a-zA-Z]+")* =>' \
  | grep -oE '"[a-zA-Z]+"' | tr -d '"' | sort -u)"
GESTURE_OVERLAP="$(comm -12 <(echo "$HOOKED") <(echo "$NEVER_ARRIVE"))"
[ -z "$GESTURE_OVERLAP" ] && r=0 || r=1
check $r "no gesture is both hooked up and declared impossible"
if [ -n "$GESTURE_OVERLAP" ]; then echo "       in both lists: $(echo "$GESTURE_OVERLAP" | tr '\n' ' ')"; fi

grep -q '"crown"' shells/watchos/Sources/AnCrown.swift && r=0 || r=1
check $r "the crown is hooked up from the shell"
grep -q 'digitalCrownRotation' shells/watchos/Sources/AnCrown.swift && r=0 || r=1
check $r "and with SwiftUI's crown API, not with an imitated gesture"

# 5. The icon table, which is now in one place.
#
#    It used to be in three — the core's, plus a private copy in `an-ios` and
#    another in `an-macos` — and this check existed to catch them drifting
#    apart: `back` has to be the same drawing on the phone, the Mac and the
#    watch. Both copies are gone, so what is checked is the stronger thing: that
#    no host has grown one back. A table nobody can duplicate cannot drift.
COPIES="$(grep -ln 'fn translate(name: &str) -> &str' crates/an-ios/src/icons.rs \
  crates/an-macos/src/icons.rs 2>/dev/null || true)"
[ -z "$COPIES" ] && r=0 || r=1
check $r "the icon table lives only in the core, with no host keeping its own"
if [ -n "$COPIES" ]; then echo "$COPIES" | sed 's/^/       /'; fi
# And that they really do go through it, rather than having stopped translating
# at all: a host that dropped the call would leave `back` reaching UIKit as the
# word "back", which is not a symbol and draws nothing.
for host in ios macos; do
  if grep -q 'an_core::icons::translate' "crates/an-$host/src/icons.rs"; then r=0; else r=1; fi
  check $r "an-$host asks the core to translate a name"
done

# 6. The shell cannot lay anything out on its own. All the layout belongs to
#    taffy, and a `VStack` or a `padding` slipped in would be a second engine
#    deciding the same thing; whichever ran later would win and nobody would know
#    why. Only the sources that paint the tree are looked at.
# Without the comments: this file explains why it does not use them, and
# explaining it cannot count as using it.
LAY_OUT="$(grep -vE '^\s*(//|\*)' shells/watchos/Sources/AnNodeView.swift shells/watchos/Sources/AnOverlays.swift \
  shells/watchos/Sources/AnBorder.swift \
  | grep -nE '\b(VStack|HStack|LazyVStack|LazyHStack|Spacer\(\)|\.padding\()' || true)"
[ -z "$LAY_OUT" ] && r=0 || r=1
check $r "the shell lays nothing out: no VStack, no HStack, no padding"
if [ -n "$LAY_OUT" ]; then echo "$LAY_OUT" | sed 's/^/       /'; fi

# 7. The examples, mounted with the viewport of a 46 mm Series 11. Without this, a
#    change in the primitives could leave the watch app unpainted and nobody
#    would find out until opening the simulator.
cargo an build examples/hello-watch >/dev/null
HELLO="$(cargo run -q -p an-bridge --example headless -- build/bundle/hello-watch/main.js 4 2>&1)"

in_hello() {
  grep -qE -- "$1" <<<"$HELLO" && r=0 || r=1
  check $r "$2"
}

in_hello 'ScrollView#[0-9]+ .*content' 'the ScrollView declares its contentSize'
in_hello 'Text#[0-9]+ .*"angular-native"' 'the heading was measured and placed'
in_hello 'Button#[0-9]+' 'the system button is mounted'
in_hello '"taps: 1"' 'the tap reached JS and the signal was recomputed'

cargo an build examples/watch-controls >/dev/null
CONTROLS="$(cargo run -q -p an-bridge --example headless -- build/bundle/watch-controls/main.js 4 2>&1)"

in_controls() {
  grep -qE -- "$1" <<<"$CONTROLS" && r=0 || r=1
  check $r "$2"
}

# That each control arrives mounted and with its state. A control that mounts
# with no props looks the same as one that does not mount: in both cases the
# layout leaves a gap.
in_controls 'Switch#[0-9]+ .*on=true' 'the switch comes down switched on'
in_controls 'Slider#[0-9]+ .*maximumValue=100' 'the slider comes down with its range'
in_controls 'Stepper#[0-9]+ .*stepValue=1' 'the stepper comes down with its step'
in_controls 'ProgressBar#[0-9]+ .*progress=0\.4' 'the bar comes down with the progress the signal computed'
in_controls 'ActivityIndicator#[0-9]+ .*animating=true' 'the spinner comes down spinning'
in_controls 'Icon#[0-9]+ .*name=favorite' 'the icon comes down with its name'
in_controls 'Alert#[0-9]+ .*buttons=' 'the dialog comes down with its buttons'
in_controls 'Modal#[0-9]+ .*presentation=sheet' 'the sheet comes down saying how it is presented'
in_controls 'StackView#[0-9]+ .*transition=' 'the stack comes down with the direction of the transition'

# The dialog takes up no room: the system presents it. If it ever did take room,
# in 248 points of height it would eat half the screen and it would not be
# obvious why.
in_controls 'Alert#[0-9]+ \[0,0 0x0\]' 'the dialog takes up no room in the layout'

# 8. The cross-compilation, which is the expensive one and the one that may not
#    be available.
if ! grep -q '^nightly' <<<"$(rustup toolchain list 2>/dev/null || true)"; then
  echo "  --   cross-compilation skipped: the nightly toolchain is missing"
  echo "       rustup toolchain install nightly"
  echo "       rustup component add rust-src --toolchain nightly"
elif ! grep -q 'rust-src (installed)' <<<"$(rustup component list --toolchain nightly 2>/dev/null || true)"; then
  echo "  --   cross-compilation skipped: rust-src is missing from nightly"
  echo "       rustup component add rust-src --toolchain nightly"
else
  if cargo +nightly build --quiet -Z build-std=std,panic_abort \
      -p an-watch --target aarch64-apple-watchos-sim 2>/dev/null; then
    echo "  ok   an-watch for aarch64-apple-watchos-sim"
  else
    echo "  FAIL an-watch does not cross-compile for aarch64-apple-watchos-sim"
    fail=1
  fi
fi

# 9. That the shell compiles.
#
#    Everything above reads the Swift with grep, which cannot tell a source
#    that builds from one that does not: a shell with a syntax error passes
#    every check here and fails on `an watchos`, where the error arrives with a
#    simulator, nightly and a build of `std` in front of it. A type check needs
#    none of that — no linking, no Rust, three seconds — so the whole shell is
#    put through the real watchOS SDK, with the same bridging header and the
#    same deployment target `an watchos` uses. The target is read out of the
#    CLI rather than written again here.
DEPLOYMENT="$(sed -n 's/^const DEPLOYMENT: &str = "\(.*\)";$/\1/p' crates/an-cli/src/watchos.rs)"
SDK_PATH="$(xcrun --sdk watchsimulator --show-sdk-path 2>/dev/null || true)"
if [ -z "$DEPLOYMENT" ]; then
  echo "  FAIL cannot read DEPLOYMENT out of crates/an-cli/src/watchos.rs"
  fail=1
elif [ -z "$SDK_PATH" ]; then
  echo "  --   the type check is skipped: there is no watchsimulator SDK on this machine"
else
  SWIFT_LOG="$(mktemp)"
  # `shells/shared` comes along because the watch's sources use it —
  # `AnBuiltinModules`, `DevClient` — and a type check of half a module is a
  # wall of undefined names rather than an answer.
  if xcrun --sdk watchsimulator swiftc -typecheck \
      -target "arm64-apple-watchos${DEPLOYMENT}-simulator" \
      -import-objc-header shells/watchos/Sources/Bridging-Header.h \
      -I crates/an-watch/include \
      shells/watchos/Sources/*.swift shells/shared/*.swift >"$SWIFT_LOG" 2>&1; then
    echo "  ok   the shell type-checks against watchOS $DEPLOYMENT"
  else
    echo "  FAIL the watch shell does not type-check"
    grep -E "error:" "$SWIFT_LOG" | head -10
    fail=1
  fi
fi

if [ "$fail" -ne 0 ]; then
  echo
  echo "$CONTROLS"
  exit 1
fi
