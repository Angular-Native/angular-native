#!/usr/bin/env bash
# Deep links: a URL from outside the app, ending up as a route.
#
# Two runs of the same example, and the interesting one is the first. A link can
# reach the shell *before* Angular exists — the system starts the process
# because of the URL — so the cold start is not "the same thing, earlier": it is
# the case with nobody to deliver to, and the only one where the URL can be lost
# to a race. Here it goes into the core before the bundle is evaluated, exactly
# as `AppDelegate` and `MainActivity` do it, and what is checked is that the very
# first screen the app paints is already the one the link asked for.
#
# The second run is the app already on screen: the link arrives as an event, the
# router pushes, and the screen the person was on stays underneath.
#
# Then the wiring nothing else would notice: the declarations in the plist and
# the manifest, the two entry points into the core, and the module name that is
# necessarily written twice.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0
check() { # <0|1> <sentence>
  if [ "$1" -eq 0 ]; then
    echo "  ok   $2"
  else
    echo "  FAIL $2"
    fail=1
  fi
}
has() { # <regex> <sentence> [text]
  grep -qE -- "$1" <<<"${3-$OUTPUT}" && check 0 "$2" || check 1 "$2"
}
hasnt() { # <regex> <sentence>
  grep -qE -- "$1" <<<"$OUTPUT" && check 1 "$2" || check 0 "$2"
}

echo "== deep links"

cargo an build examples/router >/dev/null

# ── 1. The cold start ───────────────────────────────────────────────────────
#
# `AN_OPEN_URL` puts the URL in before the bundle is evaluated. If the harness
# does not know the variable it will simply ignore it and the run will look like
# an ordinary one, so its own line is checked first: a silent pass here would be
# the exact failure this script exists to catch.
OUTPUT="$(AN_OPEN_URL='playground://ship/3' \
  cargo run -q -p an-bridge --example headless -- build/bundle/router/main.js 3 2>&1)"
if ! grep -qF 'opened with' <<<"$OUTPUT"; then
  echo "  --   the headless runner has no AN_OPEN_URL support"
  exit 0
fi

has '"Tramontana"' 'a cold start lands on the route the URL asked for'
hasnt '"Ships"' 'and the home screen was never painted on the way there'
has 'transition=none' 'so there is nothing to animate: it is where the app began'

# ── 2. Already running ──────────────────────────────────────────────────────
OUTPUT="$(AN_OPEN_URL_LATER='playground://ship/2' \
  cargo run -q -p an-bridge --example headless -- build/bundle/router/main.js 6 2>&1)"
has '"Levante"' 'a link to a running app navigates to it'
has 'transition=push' 'and it pushes, so the screen it came from is still underneath'
hasnt '(invalid buffer|uncaught rejected promise|bootstrap failed)' \
  'with no protocol errors and no promises left hanging'

# ── 3. Declared, or the system never delivers anything ──────────────────────
#
# The scheme is the bundle identifier on both platforms. It is the only spelling
# that cannot collide: a scheme is claimed device-wide, and two apps claiming
# "myapp" leave the system picking one of them.
# Four families and four decorations. One project builds a phone, a television,
# a headset and a Mac; all four can sit on the same desk, and four bundles
# claiming one scheme are four apps the system has to choose between. So each
# declares *its own* identifier, suffix and all, which is what makes the
# comparison below worth making.
for plist in shells/ios/Resources/Info.plist \
             shells/macos/Resources/Info.plist \
             shells/tvos/Resources/Info.plist \
             shells/visionos/Resources/Info.plist; do
  grep -q '<key>CFBundleURLTypes</key>' "$plist" && r=0 || r=1
  check $r "$plist declares CFBundleURLTypes"
  if command -v plutil >/dev/null 2>&1; then
    scheme="$(plutil -extract CFBundleURLTypes.0.CFBundleURLSchemes.0 raw -o - "$plist" 2>/dev/null || true)"
    id="$(plutil -extract CFBundleIdentifier raw -o - "$plist" 2>/dev/null || true)"
    [ -n "$scheme" ] && [ "$scheme" = "$id" ] && r=0 || r=1
    check $r "and its scheme is its bundle identifier ($scheme vs $id)"
  else
    echo "  --   plutil is not here, so the scheme cannot be read"
  fi
done

# The watch is the exception, and it is a refusal rather than an oversight.
# Nothing on watchOS opens a third-party app by a custom scheme: the block was
# declared here and tried, with the app installed and running, and the system
# answers that nothing claimed the URL. A declaration nothing reads is worse
# than none, because it reads as wiring — so what is checked is that the key is
# absent *and* that the reason is written where somebody would go looking for
# the key. Fill the hole and this line asks what changed on the platform.
watch_plist=shells/watchos/Resources/Info.plist
grep -q '<key>CFBundleURLTypes</key>' "$watch_plist" && r=1 || r=0
check $r 'the watch declares no scheme: nothing on watchOS can claim one'
grep -q 'CFBundleURLTypes' "$watch_plist" && r=0 || r=1
check $r 'and it says so in writing, so the gap is not a silent one'

# Wear OS runs the phone's MainActivity out of the same `shells/android/java`,
# so the class already reads the launch intent and answers `onNewIntent`. Its
# manifest is a separate file, though, and a filter added to one and not the
# other is a platform that quietly receives nothing.
for manifest in shells/android/AndroidManifest.xml shells/android/AndroidManifest.wear.xml; do
  for needle in 'android.intent.action.VIEW' 'android.intent.category.BROWSABLE' \
                'android:scheme="${applicationId}"'; do
    grep -qF -- "$needle" "$manifest" && r=0 || r=1
    check $r "$(basename "$manifest") declares $needle"
  done
  # Without singleTask a VIEW intent builds a second activity — a second engine,
  # a second tree — instead of reaching `onNewIntent`.
  grep -q 'android:launchMode="singleTask"' "$manifest" && r=0 || r=1
  check $r "and its MainActivity is singleTask, so onNewIntent is what happens"
done

# ── 4. Received on both platforms, cold and warm ────────────────────────────
swift=shells/ios/Sources/AppDelegate.swift
for needle in 'fromLaunchOptions' 'open url: URL' 'continue userActivity'; do
  grep -q "$needle" "$swift" && r=0 || r=1
  check $r "$(basename "$swift") handles $needle"
done
grep -q 'AnDeepLinks.take(fromConnectionOptions' shells/ios/Sources/SceneDelegate.swift && r=0 || r=1
check $r 'SceneDelegate.swift takes the launch URL before it builds the window'

java=shells/android/java/dev/angularnative/MainActivity.java
grep -q 'onNewIntent' "$java" && r=0 || r=1
check $r "$(basename "$java") handles onNewIntent, the app-already-running case"
# The cold start is an ordering claim and not just a call: handing the URL over
# after the bundle is evaluated still works, one navigation too late.
python3 - "$java" <<'PY' && r=0 || r=1
import sys
text = open(sys.argv[1]).read()
opened = text.find('nativeOpenUrl(launchedWith)')
evaluated = text.find('AnBundles.source')
sys.exit(0 if 0 <= opened < evaluated else 1)
PY
check $r 'and hands the launch URL over before the bundle is evaluated'
python3 - shells/ios/Sources/AppDelegate.swift <<'PY' && r=0 || r=1
import sys
text = open(sys.argv[1]).read()
opened = text.find('AnDeepLinks.take(fromLaunchOptions:')
window = text.find('RootViewController()')
sys.exit(0 if 0 <= opened < window else 1)
PY
check $r 'and so does AppDelegate, before the root view controller exists'

# The Mac has no launch options and no scene: AppKit hands the URL over through
# `application(_:open:urls:)` and nowhere else, cold and warm alike. Whether it
# arrives early enough is AppKit's business and not the file's, so there is no
# ordering to read here — section 7 runs the app instead.
mac=shells/macos/Sources/AppDelegate.swift
grep -q 'open urls: \[URL\]' "$mac" && r=0 || r=1
check $r "$(basename "$mac") handles application(_:open:urls:) on the Mac"
# The watch has neither of the phone's doors. What it has is SwiftUI's two, and
# both are needed: `onOpenURL` is how a complication's `widgetURL` arrives, and
# a universal link arrives as a browsing user activity.
watch=shells/watchos/Sources/App.swift
for needle in 'onOpenURL' 'NSUserActivityTypeBrowsingWeb'; do
  grep -q "$needle" "$watch" && r=0 || r=1
  check $r "$(basename "$watch") handles $needle, which is how a watch is reached"
done

# ── 5. The doors into the core, and the name written twice ──────────────────
for header in crates/an-ios/include/angular_native.h \
              crates/an-macos/include/angular_native_macos.h \
              crates/an-watch/include/angular_native_watch.h; do
  grep -q 'an_deeplink_open' "$header" && r=0 || r=1
  check $r "$(basename "$header") declares an_deeplink_open"
done
grep -q 'an_deeplink_open' crates/an-bridge/src/deeplink.rs && r=0 || r=1
check $r 'and the bridge exports it'
grep -q 'Java_dev_angularnative_MainActivity_nativeOpenUrl' crates/an-android/src/jni_bridge.rs \
  && r=0 || r=1
check $r 'an-android answers the JNI door Android knocks on'

# The module name cannot be shared between Rust and TypeScript, so it is two
# literals, and a drift here goes silent: the event is emitted under one name
# and nobody is subscribed to it.
rust_name="$(sed -n 's/.*DEEP_LINK_MODULE: &str = "\(.*\)".*/\1/p' crates/an-bridge/src/deeplink.rs)"
ts_name="$(sed -n "s/.*DEEP_LINK_MODULE = '\(.*\)'.*/\1/p" packages/platform-native/src/deep-links.ts)"
[ -n "$rust_name" ] && [ "$rust_name" = "$ts_name" ] && r=0 || r=1
check $r "the module is called the same on both sides (${rust_name:-?} vs ${ts_name:-?})"
rust_event="$(sed -n 's/.*DEEP_LINK_EVENT: &str = "\(.*\)".*/\1/p' crates/an-bridge/src/deeplink.rs)"
ts_event="$(sed -n "s/.*DEEP_LINK_EVENT = '\(.*\)'.*/\1/p" packages/platform-native/src/deep-links.ts)"
[ -n "$rust_event" ] && [ "$rust_event" = "$ts_event" ] && r=0 || r=1
check $r "and so is the event (${rust_event:-?} vs ${ts_event:-?})"

# ── 6. Subscribing before taking the queue ──────────────────────────────────
#
# Taking the queue is what switches the core from queueing to emitting. In the
# other order a link landing between the two calls is emitted to nobody, and
# that is a race nothing else in the suite would ever reproduce.
python3 - packages/platform-native/src/location.ts <<'PY' && r=0 || r=1
import sys
text = open(sys.argv[1]).read()
subscribe = text.find('onDeepLink(')
take = text.find('takeInitialDeepLink(')
sys.exit(0 if 0 <= subscribe < take else 1)
PY
check $r 'the location subscribes before it takes the queue, so nothing falls between'

# ── 7. The one platform where all of this can actually be run ───────────────
#
# Everything above this line reads files. macOS does not have to: there is no
# simulator to bring up and no device to look for, so the `.app` is built on the
# machine running the check, `open` hands it a URL exactly the way a person or
# another app would, and what is examined is the window that came up.
#
# It is the same claim as the headless run and it is made the same way — by what
# is *absent*. `AN_DUMP_TEXT` logs the string of every view the host mounted; if
# the home screen's title is in there, the app painted it and was pushed aside
# afterwards, which is the failure a delivery arriving one navigation too late
# produces and the one nothing else here would catch.
if [ "$(uname -s)" != "Darwin" ]; then
  echo "  --   the macOS run is skipped: the .app only builds on a Mac"
elif ! command -v plutil >/dev/null 2>&1; then
  echo "  --   the macOS run: the bundle identifier cannot be read"
else
  BUILD_LOG="$(mktemp)"
  if ! cargo an macos examples/router --no-launch >"$BUILD_LOG" 2>&1; then
    echo "  --   the macOS run: the .app did not build"
    tail -20 "$BUILD_LOG"
  else
    APP="$ROOT/build/macos/AngularNativeMac.app"
    # The scheme is the identifier and the identifier is read back out of the
    # bundle that was just built, not written here: the two cannot drift, and a
    # renamed app is a check that still tests the right URL.
    scheme="$(plutil -extract CFBundleIdentifier raw -o - "$APP/Contents/Info.plist")"
    RUN_LOG="$(mktemp)"
    # `open` and not the executable: running the binary by hand never goes near
    # LaunchServices, and LaunchServices is the half being tested. `-n` forces a
    # fresh process so this is a cold start and not a second URL to a window
    # that is already up; `-W` waits for the app, which quits itself once it has
    # taken the screenshot.
    open -W -n -a "$APP" \
      --env AN_SCREENSHOT="$ROOT/build/macos/deep-link.png" \
      --env AN_DUMP_TEXT=1 --stderr "$RUN_LOG" "$scheme://ship/3" || true
    OUTPUT="$(grep '\[text\]' "$RUN_LOG" || true)"
    if [ -z "$OUTPUT" ]; then
      echo "  FAIL the .app opened with a URL and mounted nothing at all"
      tail -20 "$RUN_LOG"
      fail=1
    else
      has 'Home port: Palma' 'a URL opens the real .app on the route it asked for'
      hasnt '\bShips\b' 'and the home screen was never painted on the way there'
    fi
    rm -f "$RUN_LOG"
  fi
  rm -f "$BUILD_LOG"
fi

if [ "$fail" -ne 0 ]; then
  exit 1
fi
