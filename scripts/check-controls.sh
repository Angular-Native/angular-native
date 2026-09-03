#!/usr/bin/env bash
# System controls: that they exist, that they measure their own size and that
# they get placed.
#
# The sizes are the ones the rough measurer returns, not the platform's: what is
# checked here is that each control is measured as a control and not as an empty
# box.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
cargo an build examples/controls >/dev/null
OUTPUT="$(cargo run -q -p an-bridge --example headless -- build/bundle/controls/main.js 3 2>&1)"

fail=0
check() {
  if grep -qE -- "$1" <<<"$OUTPUT"; then
    echo "  ok   $2"
  else
    echo "  FAIL $2"
    fail=1
  fi
}

echo "== controls"
check 'Switch#[0-9]+ \[[0-9]+,[0-9]+ 51x31\]' 'the switch measures what a switch measures'
check 'Slider#[0-9]+ \[[0-9]+,[0-9]+ 361x32\]' 'the slider stretches across and keeps its height'
check 'ProgressBar#[0-9]+ \[[0-9]+,[0-9]+ 361x4\]' 'the progress bar stretches too'
check 'ActivityIndicator#[0-9]+ \[[0-9]+,[0-9]+ 20x20\]' 'the spinner has a size of its own'
check 'Button#[0-9]+ \[[0-9]+,[0-9]+ [0-9]+x44\]' 'the button has the height of a button'
check 'TabBar#[0-9]+ \[0,803 393x49\]' 'the tab bar sticks to the bottom with its height'
check 'Modal#[0-9]+ \[0,0 393x852\]' 'the modal covers the screen'
# That the props arrive, not just that the control mounts. The dump prints them
# because a prop that arrives and one that is lost look the same in the tree:
# neither of the two changes the frame.
check 'Switch#[0-9]+ .*props .*color=#6ee7b7 .*on=true' 'the switch receives its state and its colour'
check 'ProgressBar#[0-9]+ .*props color=#6ee7b7 progress=0.35' 'the bar receives the computed progress'
# The line height and the letter spacing were measured by the core and drawn by
# no host: the layout reserved a gap the text did not fill.
check 'Text#[0-9]+ .*letterSpacing=2' 'the letter spacing reaches the host'
check 'Text#[0-9]+ .*lineHeight=34' 'the line height reaches the host'
# The configurable button: what exists on both platforms travels as an ordinary
# input, and what only one of them has travels in its own object with its
# prefix, which is what lets each host discard what is not its own.
check 'Button#[0-9]+ .*fontSize=17 fontWeight=bold' 'the button receives its typography'
check 'Button#[0-9]+ .*icon=star .*variant=filled' 'the button receives an icon and a variant'
check 'Button#[0-9]+ .*iconPosition=trailing .*variant=outlined' 'and the outlined one with the icon on the other side'
check 'Button#[0-9]+ .*enabled=true' 'a control can be switched off'
check 'Button#[0-9]+ .*ios:subtitle=full screen' 'the subtitle travels marked as an iOS one'
check 'Button#[0-9]+ .*android:allCaps=false android:rippleColor=#ffffff55' 'and the ripple and the caps as Android ones'
# The four variants with their label. The `filled` one was not drawn on iOS —it
# came out the same colour as the fill— and this row is what the screenshot the
# regression is checked against comes from. The dump does not see colours, but
# it does see that the row is still there: if somebody removes it, the
# regression loses its visual proof.
check 'Button#[0-9]+ .*title=Text variant=text' 'the text-only variant carries its label'
check 'Button#[0-9]+ .*title=Filled variant=filled' 'so does the filled one'
check 'Button#[0-9]+ .*title=Tonal variant=tonal' 'so does the tonal one'
check 'Button#[0-9]+ .*title=Outlined variant=outlined' 'and so does the outlined one'
# The switch and the slider: `[color]` is the main one —what is on, the track
# already covered— and the remaining pieces travel under their own names.
check 'Switch#[0-9]+ .*android:trackColor=#334155 color=#6ee7b7 .*thumbColor=#0b1020' 'the switch tints thumb and track separately'
check 'Slider#[0-9]+ .*maximumTrackColor=#1e2a4a .*minimumTrackColor=#6ee7b7' 'the slider tints both stretches'
check 'Slider#[0-9]+ .*android:stepSize=5 .*ios:continuous=true' 'and each platform asks for its own: steps there, reporting while dragging here'
check 'Text#[0-9]+ .*android:selectable=true .*textDecoration=underline' 'the text is underlined, and on Android it can also be copied'
check 'ScrollView#[0-9]+ .*ios:keyboardDismissMode=onDrag .*scrollEnabled=true' 'scrolling can be turned off, and on iOS the keyboard goes away on drag'
check 'TabBar#[0-9]+ .*ios:translucent=true .*unselectedColor=#6b7a99' 'unselected tabs have their colour, and on iOS the bar lets the content show through'
# The icons: that they measure what they should. 24 is the default, with no
# `[size]`; the others come from the size asked for, which also picks the
# symbol's stroke weight.
check 'Icon#[0-9]+ \[[0-9]+,[0-9]+ 24x24\]' 'an icon with no measurements measures 24'
check 'Icon#[0-9]+ \[[0-9]+,[0-9]+ 40x40\]' 'and with [size] it measures what it is asked for'

if [ "$fail" -ne 0 ]; then
  echo
  echo "$OUTPUT"
  exit 1
fi
