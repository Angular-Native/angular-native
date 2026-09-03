#!/usr/bin/env bash
# An Angular project from outside the monorepo, end to end and with no simulator.
#
# It is the path somebody takes when they have their `ng new` app and want to
# take it to the phone: `an init`, `an add ios`, `an build`. Everything is
# checked here except the last step —installing on the simulator—, which is the
# only thing that cannot be done on a machine without Xcode running.
#
# The fake project is a real project: `angular.json`, a `package.json` with
# `@angular/core`, `src/main.ts` and its web component. The only thing not done
# is downloading Angular from the network all over again: the SDK's own
# `node_modules` is cloned, and it carries the same versions. On APFS a clone
# copies no bytes and takes up no disk.
#
# Both halves are checked: that what should come out comes out, and that what
# should fail fails saying why. An `an init` on something that is not Angular, a
# repeated one that trampled the user's code, or an `Info.plist` out of step with
# the manifest are the three ways this has of ruining somebody else's project,
# and none of them may happen in silence.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"
AN="$TARGET/debug/an"
WORK="$ROOT/build/check-external"
APP="$WORK/my-app"

fail=0
ok() { echo "  ok   $1"; }
ko() { echo "  FAIL $1"; fail=1; }
contains() {
  if grep -qE -e "$2" <<<"$1"; then ok "$3"; else ko "$3"; fi
}
exists() {
  if [ -e "$1" ]; then ok "$2"; else ko "$2"; fi
}
# Runs something that has to fail, and returns its output so it can be examined.
must_fail() {
  local output
  if output="$("$@" 2>&1)"; then
    echo "EXPECTED A FAILURE AND IT SUCCEEDED: $*"
    echo "$output"
    return 1
  fi
  printf '%s' "$output"
}

echo "== Angular project from outside the monorepo"

cargo build -q -p an-cli
export AN_HOME="$ROOT"

# ---------------------------------------------------------------------------
# A real Angular project, set up by hand
# ---------------------------------------------------------------------------
rm -rf "$WORK"
mkdir -p "$APP/src/app"

cat >"$APP/package.json" <<'JSON'
{
  "name": "my-app",
  "version": "0.0.0",
  "private": true,
  "dependencies": {
    "@angular/common": "^22.1.0",
    "@angular/compiler": "^22.1.0",
    "@angular/core": "^22.1.0",
    "@angular/router": "^22.1.0",
    "rxjs": "~7.8.0"
  }
}
JSON

cat >"$APP/angular.json" <<'JSON'
{
  "$schema": "./node_modules/@angular/cli/lib/config/schema.json",
  "version": 1,
  "projects": {
    "my-app": {
      "projectType": "application",
      "root": "",
      "sourceRoot": "src",
      "architect": {
        "build": {
          "builder": "@angular/build:application",
          "options": { "browser": "src/main.ts", "index": "src/index.html" }
        }
      }
    }
  }
}
JSON

cat >"$APP/src/main.ts" <<'TS'
import { bootstrapApplication } from '@angular/platform-browser'
import { App } from './app/app'

bootstrapApplication(App)
TS

cat >"$APP/src/app/app.ts" <<'TS'
import { Component } from '@angular/core'

@Component({ selector: 'app-root', template: '<h1>the web app, untouched</h1>' })
export class App {}
TS

# The SDK's `node_modules`, cloned. `cp -c` uses clonefile on APFS: instant and
# taking up no disk. If the file system does not support it a real copy is made,
# and it is said, because then this takes half a minute and that is no mystery.
if ! cp -Rc "$ROOT/node_modules" "$APP/node_modules" 2>/dev/null; then
  echo "  (this file system does not clone; copying node_modules for real)"
  cp -R "$ROOT/node_modules" "$APP/node_modules"
fi

# ---------------------------------------------------------------------------
# What has to fail, before anything else
# ---------------------------------------------------------------------------
EMPTY="$(mktemp -d)"
trap 'rm -rf "$EMPTY"' EXIT
output="$(cd "$EMPTY" && must_fail "$AN" init)"
contains "$output" 'angular\.json' '`an init` on something that is not Angular says what is missing'
contains "$output" 'npx @angular/cli new' 'and says how a project is created'

# An Angular project that has not been initialised, outside the repo: `an` can
# guess nothing, but it does know what it is short of.
NO_INIT="$EMPTY/no-init"
mkdir -p "$NO_INIT"
cp "$APP/package.json" "$APP/angular.json" "$NO_INIT/"
output="$(cd "$NO_INIT" && must_fail "$AN" build)"
contains "$output" 'an init' 'an uninitialised Angular project is sent to `an init`'

# ---------------------------------------------------------------------------
# an init
# ---------------------------------------------------------------------------
before="$(shasum "$APP/src/app/app.ts" "$APP/src/main.ts")"
(cd "$APP" && "$AN" init >/dev/null 2>&1)

exists "$APP/angular-native.json" 'an init: writes the manifest'
exists "$APP/.angular-native/tsconfig.json" 'an init: writes the native build tsconfig'
exists "$APP/src/main.native.ts" 'an init: writes the native entry point'
exists "$APP/src/app/app-native.ts" 'an init: writes the native root component'
exists "$APP/node_modules/@angular-native/platform/dist/public-api.js" \
  'an init: installs @angular-native/platform compiled'
exists "$APP/node_modules/@angular-native/primitives/dist/public-api.js" \
  'an init: installs @angular-native/primitives compiled'
exists "$APP/node_modules/@angular-native/platform/dist/public-api.d.ts" \
  'an init: the installed package brings its types'

contains "$(cat "$APP/package.json")" 'file:\.angular-native/vendor/angular-native-platform' \
  'an init: the dependency points at the vendored tarball, not at a path on disk'
contains "$(ls "$APP/.angular-native/vendor")" '\.tgz' 'an init: the tarballs stay in the project'
contains "$(cat "$APP/.gitignore")" '^/\.angular-native/build/$' \
  'an init: only the artefact directory goes into the .gitignore'
contains "$(tar -tzf "$APP/.angular-native/vendor/"*platform*.tgz)" \
  'package/dist/public-api\.js' 'an init: the tarball carries the compiled package, not the sources'

if [ "$before" = "$(shasum "$APP/src/app/app.ts" "$APP/src/main.ts")" ]; then
  ok 'an init: does not touch the web app'
else
  ko 'an init: does not touch the web app'
fi

# The compiled package has to go in partial mode: were `ngc` to compile in
# `full`, the bundle would carry code tied to the SDK's compiler version and the
# Angular Linker would have nothing to resolve.
contains "$(cat "$APP/node_modules/@angular-native/primitives/dist/public-api.js" \
  "$APP/node_modules/@angular-native/primitives/dist/"*.js)" \
  'ɵɵngDeclare' 'an init: the packages are published in partial mode'

# Repeating it cannot ruin anything the user has written.
echo "// the user edited this" >>"$APP/src/app/app-native.ts"
fingerprint="$(shasum "$APP/src/app/app-native.ts")"
(cd "$APP" && "$AN" init >/dev/null 2>&1)
if [ "$fingerprint" = "$(shasum "$APP/src/app/app-native.ts")" ]; then
  ok 'an init repeated: it respects the code that was already there'
else
  ko 'an init repeated: it respects the code that was already there'
fi

# ---------------------------------------------------------------------------
# an add
# ---------------------------------------------------------------------------
(cd "$APP" && "$AN" add ios >/dev/null 2>&1)
exists "$APP/ios/Info.plist" 'an add ios: creates the project Info.plist'
contains "$(plutil -extract CFBundleExecutable raw -o - "$APP/ios/Info.plist")" '^MyApp$' \
  "an add ios: the plist's executable is the app's name"
contains "$(plutil -extract CFBundleIdentifier raw -o - "$APP/ios/Info.plist")" \
  '^dev\.angularnative\.myapp$' "an add ios: the plist's identifier comes from the manifest"
contains "$(cat "$APP/angular-native.json")" '"ios"' 'an add ios: it is noted down in the manifest'

echo "<!-- the user added this -->" >>"$APP/ios/Info.plist"
fingerprint="$(shasum "$APP/ios/Info.plist")"
(cd "$APP" && "$AN" add ios >/dev/null 2>&1)
if [ "$fingerprint" = "$(shasum "$APP/ios/Info.plist")" ]; then
  ok "an add ios repeated: it does not trample the user's plist"
else
  ko "an add ios repeated: it does not trample the user's plist"
fi
# And remove the test line: a comment after </plist> is no longer a valid plist,
# and what comes afterwards is read by `plutil`.
sed -i '' -e '$d' "$APP/ios/Info.plist"

(cd "$APP" && "$AN" add android >/dev/null 2>&1)
exists "$APP/android/AndroidManifest.xml" 'an add android: creates the project manifest'
contains "$(cat "$APP/android/AndroidManifest.xml")" 'android:label="MyApp"' \
  "an add android: the label is the app's name"
contains "$(cat "$APP/android/AndroidManifest.xml")" 'package="dev\.angularnative"' \
  "an add android: the package is still the shell classes'"

output="$(cd "$APP" && must_fail "$AN" add windows)"
# The list of platforms `an add` claims to know has to be the one it really
# knows: it is the first thing read by whoever gets the name wrong, and one
# missing from it is one nobody is going to try.
#
# The conjunction is the CLI's own word and the CLI is a crate translated on
# another branch, so both are accepted.
contains "$output" 'ios, tvos, visionos (y|and) android' \
  'an add of a platform that does not exist says which ones it knows'

# ---------------------------------------------------------------------------
# an build
# ---------------------------------------------------------------------------
(cd "$APP" && "$AN" build >/dev/null 2>&1)
BUNDLE="$APP/.angular-native/build/bundle/main.js"
exists "$BUNDLE" 'an build: the bundle comes out, inside the project and not the SDK'
if [ -e "$ROOT/build/bundle/my-app" ]; then
  ko 'an build: it writes nothing into the SDK'
else
  ok 'an build: it writes nothing into the SDK'
fi

# And that the bundle runs: the same `headless` the other scripts use, which
# mounts the whole pipeline minus the platform.
output="$(cargo run -q -p an-bridge --example headless -- "$BUNDLE" 3 2>&1)"
contains "$output" 'Angular is running in development mode' 'the bundle starts Angular'
contains "$output" 'Text#[0-9]+ .*"MyApp"' "the app's title reached a native Text node"
# The button's label is written by the scaffold the CLI generates, and the CLI is
# a crate translated on another branch. What is checked is the count, which is
# what proves the tap arrived: before it, the title carries a zero.
contains "$output" 'Button#[0-9]+ .*title=.*1' 'a tap made it all the way to the component signal'

# ---------------------------------------------------------------------------
# The plist and the manifest, out of step
# ---------------------------------------------------------------------------
# Changing the app's name and not touching the plist leaves an app that installs
# and does not open: iOS looks for an executable that is not there. It has to
# stop before compiling.
sed -i '' -e 's/"name": "MyApp"/"name": "AnotherName"/' "$APP/angular-native.json"
output="$(cd "$APP" && must_fail "$AN" ios --no-launch)"
contains "$output" 'CFBundleExecutable' 'a plist that does not match the manifest stops the build'
sed -i '' -e 's/"name": "AnotherName"/"name": "MyApp"/' "$APP/angular-native.json"

if [ "$fail" -ne 0 ]; then
  exit 1
fi
