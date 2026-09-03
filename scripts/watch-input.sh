#!/usr/bin/env bash
# Tap the screen and turn the crown of the watchOS simulator from outside.
#
# `xcrun simctl` has no verb for this: there are `io … screenshot`,
# `io … recordVideo`, `ui`, `spawn`, `push`… and none of them sends a tap or a
# turn. The same thing that happened with the Apple TV remote, and the way out is
# the same as in `tv-remote.sh`: move Simulator's own mouse and keyboard.
#
#   ./scripts/watch-input.sh tap 104 200          tap at (104,200), in points
#   ./scripts/watch-input.sh drag 104 220 104 60  drag: scrolls a list
#   ./scripts/watch-input.sh hold 104 120 900     press and hold for 900 ms
#   ./scripts/watch-input.sh turn 12              twelve crown steps downwards
#   ./scripts/watch-input.sh turn -12             and upwards
#   ./scripts/watch-input.sh shot /tmp/a.png      screenshot
#
# The coordinates are **watch points**, not pixels of the Mac's screen: the
# window is found by its name and the screen rectangle is read from the
# accessibility tree, so it does not matter where the window is and it does not
# matter that there are other simulators open.
#
# The crown was the hardest thing to find and that is why it is written down:
# **the mouse wheel only turns the crown if the pointer is over the window's
# "Crown" button**, not over the screen. Over the screen absolutely nothing
# happens —not even a warning— and it is easy to conclude that the crown cannot
# be moved from outside, which is what happened here.
#
# It has the same two conditions as the television remote, and for the same
# reasons: **the Mac's screen cannot be locked** —with the session locked no
# application can be brought to the front and the events do not arrive— and
# **Simulator stays in the foreground** while this runs, because the mouse is
# its. With another simulator in front, that one takes the events.
#
# `AN_WATCH_WINDOW` changes what the window is searched for by; by default,
# anything saying "Watch", "Series" or "Ultra", which is how watchOS 26 names
# them.
set -euo pipefail

WINDOW="${AN_WATCH_WINDOW:-}"
UDID="${AN_WATCH_UDID:-booted}"

usage() {
  sed -n '2,32p' "$0" | sed 's/^# \{0,1\}//'
  exit 2
}

[ $# -ge 1 ] || usage
COMMAND="$1"
shift

if [ "$COMMAND" = "shot" ]; then
  [ $# -eq 1 ] || usage
  xcrun simctl io "$UDID" screenshot "$1" >/dev/null 2>&1
  echo "screenshot at $1"
  exit 0
fi

# ---------------------------------------------------------------- the window

if [ -n "$WINDOW" ]; then
  FILTER="name of w contains \"$WINDOW\""
else
  FILTER='name of w contains "Watch" or name of w contains "Series" or name of w contains "Ultra"'
fi

osascript -e 'tell application "Simulator" to activate' >/dev/null
# Coming to the front is not instant, and an event that arrives before it does
# goes to another window without a word.
sleep 0.6

RECT="$(osascript <<AS
tell application "System Events"
  if not (exists process "Simulator") then error "Simulator is not open"
  tell process "Simulator"
    repeat with w in windows
      if $FILTER then
        -- The watch screen and the crown button, in the Mac's screen
        -- coordinates. Both come out of the accessibility tree rather than being
        -- assumed: the window can be moved and it can be scaled.
        set g to group 1 of group 1 of w
        set {px, py} to position of g
        set {sw, sh} to size of g
        set {cx, cy} to position of button "Crown" of w
        set {cw, ch} to size of button "Crown" of w
        -- Integers on purpose: on a Mac with the decimal comma, "500 / 2" is
        -- written "250,0" and whatever reads it on the other side sees two
        -- fields.
        return (px as text) & " " & (py as text) & " " & (sw as text) & " " & (sh as text) & " " & ((cx + (cw div 2)) as text) & " " & ((cy + (ch div 2)) as text)
      end if
    end repeat
    error "there is no watch window in Simulator"
  end tell
end tell
AS
)"
read -r OX OY OW OH CX CY <<<"$RECT"

# The watch's size in points. `simctl io … enumerate` gives it in pixels and
# every watch is 2x, so it is halved. It is needed because the Simulator window
# can be scaled: without this, a tap at (104,200) with the window at 75 % would
# go thirty points lower than the template says.
PIXELS="$(xcrun simctl io "$UDID" enumerate 2>/dev/null | awk '/Default width:/{w=$3} /Default height:/{h=$3} END{print w+0, h+0}')"
read -r PX_W PX_H <<<"$PIXELS"

point() { # x y in watch points -> x y on the Mac's screen
  python3 "$POINT_HELPER" "$1" "$2" "$OX" "$OY" "$OW" "$OH" "$PX_W" "$PX_H"
}

POINT_HELPER="$(mktemp -t watch-point).py"
cat > "$POINT_HELPER" <<'PY'
import sys

x, y, ox, oy, ow, oh, pxw, pxh = (float(v) for v in sys.argv[1:9])
# With no device measurements, 1:1 is assumed, which is what Simulator does with
# the window at its natural size.
width = pxw / 2 if pxw else ow
height = pxh / 2 if pxh else oh
print(round(ox + x * ow / width), round(oy + y * oh / height))
PY

SW_FILE="$(mktemp -t watch-input).swift"
trap 'rm -f "$SW_FILE" "$POINT_HELPER"' EXIT
cat > "$SW_FILE" <<'SWIFT'
// The mouse through CoreGraphics. `cliclick` is not used so as not to ask for
// one more homebrew: dragging and turning the wheel are four CGEvent calls.
import CoreGraphics
import Foundation

let a = CommandLine.arguments

func move(_ p: CGPoint) {
    CGEvent(mouseEventSource: nil, mouseType: .mouseMoved, mouseCursorPosition: p, mouseButton: .left)?
        .post(tap: .cghidEventTap)
}

func button(_ type: CGEventType, _ p: CGPoint) {
    CGEvent(mouseEventSource: nil, mouseType: type, mouseCursorPosition: p, mouseButton: .left)?
        .post(tap: .cghidEventTap)
}

switch a[1] {
case "tap":
    let p = CGPoint(x: Double(a[2])!, y: Double(a[3])!)
    move(p); usleep(120_000)
    button(.leftMouseDown, p); usleep(80_000)
    button(.leftMouseUp, p)
case "hold":
    // Press and hold without moving. The watchOS threshold is around half a
    // second, so the usual thing is to ask for more.
    let p = CGPoint(x: Double(a[2])!, y: Double(a[3])!)
    let ms = UInt32(a[4])!
    move(p); usleep(150_000)
    button(.leftMouseDown, p)
    usleep(ms * 1000)
    button(.leftMouseUp, p)
case "drag":
    let from = CGPoint(x: Double(a[2])!, y: Double(a[3])!)
    let to = CGPoint(x: Double(a[4])!, y: Double(a[5])!)
    move(from); usleep(150_000)
    button(.leftMouseDown, from); usleep(80_000)
    // In steps: a single-event jump is read by the simulator as a teleport and
    // is not turned into a drag.
    for i in 1...12 {
        let t = Double(i) / 12.0
        let p = CGPoint(
            x: from.x + (to.x - from.x) * t,
            y: from.y + (to.y - from.y) * t
        )
        button(.leftMouseDragged, p)
        usleep(20_000)
    }
    button(.leftMouseUp, to)
case "turn":
    // Over the crown button, not over the screen: over the screen the wheel does
    // nothing.
    let p = CGPoint(x: Double(a[2])!, y: Double(a[3])!)
    let steps = Int(a[4])!
    move(p); usleep(200_000)
    let sign: Int32 = steps < 0 ? -1 : 1
    for _ in 0..<abs(steps) {
        guard let e = CGEvent(
            scrollWheelEvent2Source: nil, units: .line, wheelCount: 1,
            wheel1: sign, wheel2: 0, wheel3: 0
        ) else { continue }
        e.location = p
        e.post(tap: .cghidEventTap)
        usleep(45_000)
    }
default:
    FileHandle.standardError.write("unknown command\n".data(using: .utf8)!)
    exit(2)
}
SWIFT

case "$COMMAND" in
  tap)
    [ $# -eq 2 ] || usage
    read -r X Y <<<"$(point "$1" "$2")"
    swift "$SW_FILE" tap "$X" "$Y"
    echo "tap at ($1,$2) -> screen ($X,$Y)"
    ;;
  hold)
    [ $# -eq 3 ] || usage
    read -r X Y <<<"$(point "$1" "$2")"
    swift "$SW_FILE" hold "$X" "$Y" "$3"
    echo "long press at ($1,$2) for $3 ms"
    ;;
  drag)
    [ $# -eq 4 ] || usage
    read -r X1 Y1 <<<"$(point "$1" "$2")"
    read -r X2 Y2 <<<"$(point "$3" "$4")"
    swift "$SW_FILE" drag "$X1" "$Y1" "$X2" "$Y2"
    echo "drag ($1,$2) -> ($3,$4)"
    ;;
  turn)
    [ $# -eq 1 ] || usage
    swift "$SW_FILE" turn "$CX" "$CY" "$1"
    echo "crown: $1 steps"
    ;;
  *)
    usage
    ;;
esac
