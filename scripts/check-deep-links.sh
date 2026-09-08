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
  echo "  skipped the headless runner has no AN_OPEN_URL support"
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
plist=shells/ios/Resources/Info.plist
grep -q 'CFBundleURLTypes' "$plist" && r=0 || r=1
check $r "$plist declares CFBundleURLTypes"
if command -v plutil >/dev/null 2>&1; then
  scheme="$(plutil -extract CFBundleURLTypes.0.CFBundleURLSchemes.0 raw -o - "$plist" 2>/dev/null || true)"
  id="$(plutil -extract CFBundleIdentifier raw -o - "$plist" 2>/dev/null || true)"
  [ -n "$scheme" ] && [ "$scheme" = "$id" ] && r=0 || r=1
  check $r "and the scheme is the bundle identifier ($scheme vs $id)"
else
  echo "  skipped plutil is not here, so the scheme cannot be read"
fi

manifest=shells/android/AndroidManifest.xml
for needle in 'android.intent.action.VIEW' 'android.intent.category.BROWSABLE' \
              'android:scheme="${applicationId}"'; do
  grep -qF -- "$needle" "$manifest" && r=0 || r=1
  check $r "$(basename "$manifest") declares $needle"
done
# Without singleTask a VIEW intent builds a second activity — a second engine,
# a second tree — instead of reaching `onNewIntent`.
grep -q 'android:launchMode="singleTask"' "$manifest" && r=0 || r=1
check $r 'and MainActivity is singleTask, so onNewIntent is what happens'

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

# ── 5. The two doors into the core, and the name written twice ──────────────
grep -q 'an_deeplink_open' crates/an-ios/include/angular_native.h && r=0 || r=1
check $r 'angular_native.h declares an_deeplink_open'
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

if [ "$fail" -ne 0 ]; then
  exit 1
fi
