#!/usr/bin/env bash
# The Apple TV remote, from the command line.
#
#   ./scripts/tv-remote.sh down down select
#
# `xcrun simctl` has no verb for the remote: it has `io ... screenshot`,
# `io ... recordVideo`, `ui`, `spawn` and `push`, and none of them sends a press;
# the binary hides nothing like it either. What there is is Simulator's own
# keyboard input, which the tvOS simulator translates into focus movements and
# the centre button. So this is `osascript` sending key codes, and it does not
# pretend to be anything else.
#
# Three conditions that cannot be dodged, and that is why they are checked before
# sending anything: if they fail, the press would go somewhere else and nobody
# here would find out.
#
#   1. The Mac's screen cannot be locked. With the session locked no application
#      can be brought to the front.
#   2. Simulator has to end up in the foreground. While this runs, the keyboard
#      is its.
#   3. The Apple TV window has to be the one with the focus *inside* Simulator.
#      With an iPhone open at the same time, the keys are taken by whichever
#      window was in front, which is a different simulator, and in the Apple TV's
#      nothing at all happens.
#
# `AN_TV_WINDOW` changes what that window is searched for by; by default, "tvOS",
# which is what Simulator puts in the title of every window of that family.
set -euo pipefail

WINDOW="${AN_TV_WINDOW:-tvOS}"

if [ "$#" -eq 0 ]; then
  echo "usage: $0 <key>..." >&2
  echo "       keys: up down left right select menu" >&2
  exit 2
fi

# macOS key codes. The arrow ones are what the tvOS simulator turns into focus
# movements; return is the centre button and escape is the menu button, which is
# the remote's "back".
key() {
  case "$1" in
    up) echo 126 ;;
    down) echo 125 ;;
    left) echo 123 ;;
    right) echo 124 ;;
    select | ok | enter) echo 36 ;;
    menu | back) echo 53 ;;
    *)
      echo "I do not know the key \"$1\"; there are up, down, left, right, select and menu" >&2
      exit 2
      ;;
  esac
}

# They are all validated before any of them is sent: half a sequence sent and
# then an error leaves the focus halfway and the next screenshot lies.
for name in "$@"; do
  key "$name" >/dev/null
done

if grep -q "CGSSessionScreenIsLocked" <<<"$(ioreg -n Root -d1 -a 2>/dev/null || true)"; then
  echo "the Mac's screen is locked: no key would reach the simulator." >&2
  echo "Unlock it and try again." >&2
  exit 1
fi

if ! grep -q "tvOS"  <<<"$(xcrun simctl list devices booted 2>/dev/null || true)"; then
  echo "there is no tvOS simulator running." >&2
  echo "Start the app with: cargo an tvos" >&2
  exit 1
fi

osascript -e 'tell application "Simulator" to activate' >/dev/null
front=""
for _ in 1 2 3 4 5 6 7 8 9 10; do
  front="$(osascript -e 'tell application "System Events" to return name of first application process whose frontmost is true')"
  [ "$front" = "Simulator" ] && break
  sleep 0.3
done
if [ "$front" != "Simulator" ]; then
  echo "Simulator did not reach the foreground (\"$front\" is in front)." >&2
  echo "Without that the keys would go to that other application, so none is sent." >&2
  exit 1
fi

# The Apple TV window, in front of Simulator's other windows.
if ! osascript -e "tell application \"System Events\" to tell application process \"Simulator\" \
    to tell (first window whose title contains \"$WINDOW\") to perform action \"AXRaise\"" \
    >/dev/null 2>&1; then
  echo "Simulator has no window whose title contains \"$WINDOW\"." >&2
  exit 1
fi
sleep 0.5
focused="$(osascript -e 'tell application "System Events" to tell application process "Simulator" to return title of (first window whose focused is true)' 2>/dev/null || echo '')"
case "$focused" in
  *"$WINDOW"*) ;;
  *)
    echo "the focused window inside Simulator is \"$focused\", not one of $WINDOW." >&2
    echo "The keys would be taken by that other simulator, so none is sent." >&2
    exit 1
    ;;
esac

for name in "$@"; do
  code="$(key "$name")"
  osascript -e "tell application \"System Events\" to key code $code" >/dev/null
  # The focus engine animates the jump; chaining without waiting swallows presses.
  sleep 0.6
done
