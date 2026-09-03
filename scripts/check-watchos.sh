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
if cargo test --quiet -p an-watch >/dev/null 2>&1; then
  echo "  ok   the model SwiftUI sees is built as it should be"
else
  echo "  FAIL the an-watch tests do not pass"
  cargo test -p an-watch 2>&1 | tail -20
  fail=1
fi

# 2. The three lists of primitives.
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

# 3. The gestures. What the shell hooks up and what the host says never arrives
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

# 4. The icon table, which lives in two places while `an-ios` keeps its own.
#    Copied is allowed; drifted is not: `back` has to be the same drawing on the
#    phone and on the watch.
table() {
  awk '/fn translate/,/^\}/' "$1" | grep -oE '"[^"]+"( \| "[^"]+")* => "[^"]+"' | sort
}
diff <(table crates/an-core/src/icons.rs) <(table crates/an-ios/src/icons.rs) >/dev/null 2>&1 && r=0 || r=1
check $r "the core's icon table says the same as an-ios's"

# 5. The shell cannot lay anything out on its own. All the layout belongs to
#    taffy, and a `VStack` or a `padding` slipped in would be a second engine
#    deciding the same thing; whichever ran later would win and nobody would know
#    why. Only the sources that paint the tree are looked at.
# Without the comments: this file explains why it does not use them, and
# explaining it cannot count as using it.
LAY_OUT="$(grep -vE '^\s*(//|\*)' shells/watchos/Sources/AnNodeView.swift shells/watchos/Sources/AnOverlays.swift \
  | grep -nE '\b(VStack|HStack|LazyVStack|LazyHStack|Spacer\(\)|\.padding\()' || true)"
[ -z "$LAY_OUT" ] && r=0 || r=1
check $r "the shell lays nothing out: no VStack, no HStack, no padding"
if [ -n "$LAY_OUT" ]; then echo "$LAY_OUT" | sed 's/^/       /'; fi

# 6. The examples, mounted with the viewport of a 46 mm Series 11. Without this, a
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

# 7. The cross-compilation, which is the expensive one and the one that may not
#    be available.
if ! rustup toolchain list 2>/dev/null | grep -q '^nightly'; then
  echo "  --   cross-compilation skipped: the nightly toolchain is missing"
  echo "       rustup toolchain install nightly"
  echo "       rustup component add rust-src --toolchain nightly"
elif ! rustup component list --toolchain nightly 2>/dev/null | grep -q 'rust-src (installed)'; then
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

if [ "$fail" -ne 0 ]; then
  echo
  echo "$CONTROLS"
  exit 1
fi
