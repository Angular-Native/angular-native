#!/usr/bin/env bash
# `an dev --macos`: the whole development loop, on the one platform where it can
# be watched from end to end without anybody looking at a screen.
#
# macOS is not a simulator. The app runs on the machine that compiled it, so the
# server, the compiler, the window and this script are all the same computer:
# the bundle can be served, the `.app` built with the server's address inside it,
# the app started, a source file saved, and then the window asked what it is
# showing. Nothing here is simulated and nothing is assumed.
#
# And what is asked of it is the part that makes the feature worth having. A
# reload that swaps the code is easy; a reload that swaps the code **and leaves
# the app where it was** is the whole point, and the two look identical from
# outside unless the state is read. `hello-angular` puts a running timer and an
# `@if` that unfolds at three seconds on screen, and a restart sends both back to
# zero. `AN_DUMP_TEXT=1` makes the shell log the strings its `NSView`s are really
# showing —see `shells/macos/Sources/TextDump.swift`—, so the check reads the
# window and not the model behind it.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== an dev --macos"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "  --   skipped: the macOS host only builds on a Mac"
  exit 0
fi

fail=0
check() { # <0 if good, 1 if bad> <what was being checked>
  if [ "$1" -eq 0 ]; then
    echo "  ok   $2"
  else
    echo "  FAIL $2"
    fail=1
  fi
}

# 1. The flag exists. It is checked against `--help` and not against the source
#    because what matters is that clap accepts it: a variant added to the enum
#    and forgotten in the command line is exactly the shape this hole had.
if grep -q -- '--macos' <<<"$(cargo an dev --help 2>&1 || true)"; then
  echo "  ok   the flag is on the command line"
else
  echo '  FAIL there is no --macos in an dev'
  exit 1
fi

# A port of its own, so a dev server somebody left running does not turn this
# into a mystery. If it is taken, that is said instead of failing further along.
PORT=8431
if lsof -ti "tcp:$PORT" >/dev/null 2>&1; then
  echo "  --   skipped: something is already listening on port $PORT"
  exit 0
fi

APP="$ROOT/build/macos/AngularNativeMac.app"
BIN="$APP/Contents/MacOS/AngularNativeMac"
URL_FILE="$APP/Contents/Resources/dev-server.txt"
SOURCE="examples/hello-angular/src/app.component.ts"
BACKUP="$(mktemp)"
DEV_LOG="$(mktemp)"
APP_LOG="$(mktemp)"
SHOT="$ROOT/build/macos/dev-reload.png"
cp "$SOURCE" "$BACKUP"

DEV_PID=""
APP_PID=""
cleanup() {
  [ -n "$APP_PID" ] && kill "$APP_PID" 2>/dev/null || true
  [ -n "$DEV_PID" ] && kill "$DEV_PID" 2>/dev/null || true
  # `cargo run` is the parent of the real binary, so killing it leaves the
  # server holding the port. Whoever is on the port goes too, and only that one.
  for pid in $(lsof -ti "tcp:$PORT" 2>/dev/null); do kill -9 "$pid" 2>/dev/null || true; done
  killall -9 AngularNativeMac 2>/dev/null || true
  cp "$BACKUP" "$SOURCE"
  rm -f "$BACKUP" "$DEV_LOG" "$APP_LOG"
}
trap cleanup EXIT

# 2. The server, with the app it builds. This is the real subcommand: it
#    compiles the bundle, assembles the `.app` with the URL inside it, opens it
#    and stays watching the sources.
rm -f "$URL_FILE"
cargo an dev examples/hello-angular --macos --port "$PORT" >"$DEV_LOG" 2>&1 &
DEV_PID=$!

waited=0
while [ ! -s "$URL_FILE" ]; do
  if ! kill -0 "$DEV_PID" 2>/dev/null; then
    echo "  FAIL the dev server died before building the .app"
    tail -30 "$DEV_LOG"
    exit 1
  fi
  sleep 1
  waited=$((waited + 1))
  if [ "$waited" -gt 300 ]; then
    echo "  FAIL the .app was never built with a dev-server.txt in it"
    tail -30 "$DEV_LOG"
    exit 1
  fi
done
echo "  ok   the .app is built with the server's address inside it"

# 3. And the address is this machine's. It is `127.0.0.1` on every target now —
#    Android reaches it through `adb reverse` rather than through a translated
#    host — but a Mac app is the one that could never have needed anything else:
#    it is not inside anything, it runs on the machine serving the bundle.
URL="$(cat "$URL_FILE")"
[ "$URL" = "http://127.0.0.1:$PORT" ] && r=0 || r=1
check $r "the URL is plain 127.0.0.1, with no emulator translation ($URL)"

# The window `open` put up belongs to nobody: it was launched by LaunchServices
# and carries none of the environment this check needs. It is taken down and the
# same binary is started again from here, which is the only way to hand it
# `AN_DUMP_TEXT`.
waited=0
while ! pgrep -x AngularNativeMac >/dev/null 2>&1; do
  sleep 1
  waited=$((waited + 1))
  [ "$waited" -gt 60 ] && break
done
killall -9 AngularNativeMac 2>/dev/null || true
while pgrep -x AngularNativeMac >/dev/null 2>&1; do sleep 1; done

# 4. The app, ours this time. It waits for one reload and photographs what is on
#    screen afterwards, which is the frame that shows whether the tree survived.
AN_SCREENSHOT="$SHOT" AN_SCREENSHOT_RELOADS=1 AN_SCREENSHOT_FRAMES=90 AN_DUMP_TEXT=1 \
  "$BIN" >"$APP_LOG" 2>&1 &
APP_PID=$!

waited=0
while ! grep -q "connected to the development server" "$APP_LOG"; do
  if ! kill -0 "$APP_PID" 2>/dev/null; then
    echo "  FAIL the app died before reaching the server"
    tail -30 "$APP_LOG"
    exit 1
  fi
  sleep 1
  waited=$((waited + 1))
  if [ "$waited" -gt 60 ]; then
    echo "  FAIL the app never connected to the development server"
    tail -30 "$APP_LOG"
    exit 1
  fi
done
echo "  ok   the app connects to the server on its own"

# 5. Something to lose. Seven seconds of the app's own clock: the timer is past
#    six and the `@if` unfolded four seconds ago. Neither survives a restart.
sleep 7

# 6. The save. The same sentence `check-hot.sh` swaps, so the two say the same
#    thing about the same example.
sed -i '' 's/This is an Angular template with signals, running on QuickJS./TEMPLATE CHANGED WHILE HOT./' "$SOURCE"

# 7. The app fires the shutter on its own once the reload has landed and exits.
waited=0
while kill -0 "$APP_PID" 2>/dev/null; do
  sleep 1
  waited=$((waited + 1))
  if [ "$waited" -gt 180 ]; then
    echo "  FAIL nothing reached the app in three minutes after saving"
    tail -30 "$DEV_LOG"
    tail -30 "$APP_LOG"
    exit 1
  fi
done
APP_PID=""

grep -q "angular-native: reloading" "$APP_LOG" && r=0 || r=1
check $r "saving reaches the running app"

# 8. What the window is showing, read off the window.
in_window() { grep -qF -- "[text] $1" "$APP_LOG"; }

in_window "TEMPLATE CHANGED WHILE HOT." && r=0 || r=1
check $r "the new template is on screen"

in_window "This is an Angular template with signals" && r=1 || r=0
check $r "and the old one is not underneath it"

# The two that a restart cannot fake.
in_window "The @if came in at 3 seconds." && r=0 || r=1
check $r "what had already unfolded is still unfolded"

SECONDS_SHOWN="$(sed -n 's/.*\[text\] seconds running: \([0-9]*\).*/\1/p' "$APP_LOG" | tail -1)"
if [ -n "$SECONDS_SHOWN" ] && [ "$SECONDS_SHOWN" -ge 6 ]; then
  echo "  ok   the timer kept running across the save (seconds running: $SECONDS_SHOWN)"
else
  echo "  FAIL the state did not survive: seconds running came back as ${SECONDS_SHOWN:-nothing}"
  fail=1
fi

# 9. And that there is still a window. A hot reload that unmounts the views
#    leaves everything above passing —the strings are gone, so they cannot
#    contradict anything— and a black rectangle on screen.
COLOURS="$(sed -n 's/.*, \([0-9]*\) colours).*/\1/p' "$APP_LOG" | tail -1)"
if [ -s "$SHOT" ] && [ -n "$COLOURS" ] && [ "$COLOURS" -gt 16 ]; then
  echo "  ok   the window is still painted after the reload ($COLOURS colours)"
else
  echo "  FAIL the window came out blank after the reload"
  fail=1
fi

if [ "$fail" -ne 0 ]; then
  echo
  cat "$DEV_LOG"
  cat "$APP_LOG"
  exit 1
fi
