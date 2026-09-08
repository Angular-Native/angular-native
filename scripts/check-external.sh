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
#
# The last section is the same three commands over again with the one difference
# that matters to anybody who is not us: the `an` is the one `npm install -g
# @angular-native/cli` puts on the PATH, the SDK is the payload under a
# `node_modules`, and there is no checkout and no `AN_HOME`.
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

# The project every part of this script starts from. It is written out rather
# than generated because `ng new` would want the network and half a minute, and
# what `an` reads is these four files.
#
# `@angular/platform-browser` and `typescript` are in the list on purpose even
# though nothing here imports the first and no source is compiled by hand with
# the second. `ng new` puts both in, and leaving them out only works under a
# manager that installs peer dependencies by itself: yarn does not, and the
# bundle then dies on `Could not resolve "@angular/platform-browser"` from
# inside `@angular/router`, which is the project's gap and not `an`'s.
scaffold_project() { # $1 = the directory to write the project into
  local dir="$1"
  mkdir -p "$dir/src/app"

  cat >"$dir/package.json" <<'JSON'
{
  "name": "my-app",
  "version": "0.0.0",
  "private": true,
  "dependencies": {
    "@angular/common": "^22.1.0",
    "@angular/compiler": "^22.1.0",
    "@angular/core": "^22.1.0",
    "@angular/platform-browser": "^22.1.0",
    "@angular/router": "^22.1.0",
    "rxjs": "~7.8.0"
  },
  "devDependencies": {
    "typescript": "6.0.3"
  }
}
JSON

  cat >"$dir/angular.json" <<'JSON'
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

  cat >"$dir/src/main.ts" <<'TS'
import { bootstrapApplication } from '@angular/platform-browser'
import { App } from './app/app'

bootstrapApplication(App)
TS

  cat >"$dir/src/app/app.ts" <<'TS'
import { Component } from '@angular/core'

@Component({ selector: 'app-root', template: '<h1>the web app, untouched</h1>' })
export class App {}
TS
}

scaffold_project "$APP"

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
  'an init: the artefact directory goes into the .gitignore'
# And the credential patterns. Not tidiness: a release keystore in a repository
# is the app's whole identity on Google Play, and the only moment adding a line
# to the .gitignore costs nothing is before there is anything to catch.
for pattern in keystore jks p12 mobileprovision; do
  contains "$(cat "$APP/.gitignore")" "^[*]\\.$pattern\$" \
    "an init: *.$pattern is ignored, so that credential cannot be committed by accident"
done
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
contains "$output" 'ios, tvos, visionos, macos and android' \
  'an add of a platform that does not exist says which ones it knows'

# ── macOS, which used to be the one platform a project could not name ───────
#
# `an macos` ignored the project's name and identifier outright: every Mac app
# anybody built came out called AngularNativeMac and identified as the
# playground, so two apps from two projects were one app as far as the system
# was concerned — one container, one Dock entry, each replacing the other.
output="$(cd "$APP" && "$AN" add macos 2>&1)"
exists "$APP/macos/Info.plist" 'an add macos: creates the project Info.plist'
plist_value() {
  plutil -extract "$1" raw -o - "$APP/macos/Info.plist" 2>/dev/null | tr -d '\n'
}
if [ "$(plist_value CFBundleExecutable)" = "MyAppMac" ]; then
  ok "an add macos: the executable is the app's name plus Mac, like tvOS's plus TV"
else
  ko "an add macos: the executable is the app's name plus Mac, like tvOS's plus TV"
fi
if [ "$(plist_value CFBundleIdentifier)" = "dev.angularnative.myapp.mac" ]; then
  ok "an add macos: the identifier comes from the manifest"
else
  ko "an add macos: the identifier comes from the manifest"
fi
contains "$(cat "$APP/angular-native.json")" '"macos"' \
  'an add macos: it is noted down in the manifest'

# ── What the app looks like ─────────────────────────────────────────────────
#
# One word in the manifest, three platform idioms underneath. What is worth
# checking from out here is that the word travels at all, and that a word none
# of them knows stops the build: the Android shell used to force dark on every
# app built with it, so the failure this replaces is a setting that is read,
# written, and quietly means nothing.
contains "$(cat "$APP/angular-native.json")" '"appearance": "system"' \
  'an init: the manifest says what the app looks like, and it follows the device'

appearance() {
  node -e '
    const fs = require("fs"), path = process.argv[1];
    const manifest = JSON.parse(fs.readFileSync(path, "utf8"));
    manifest.app.appearance = process.argv[2];
    fs.writeFileSync(path, JSON.stringify(manifest, null, 2) + "\n");
  ' "$APP/angular-native.json" "$1"
}

appearance midnight
output="$(cd "$APP" && must_fail "$AN" build)"
contains "$output" 'app.appearance is "midnight"' \
  'a word none of the platforms knows stops the build rather than meaning system'
contains "$output" '"light"' 'and the refusal lists the three that do work'
appearance system
# The same rule the other platforms have, checked the same way: something the
# user wrote has to survive. A `$(cat)` will not do here — it eats the trailing
# newline and the comparison then fails on a file nobody touched.
echo "<!-- the user added this -->" >>"$APP/macos/Info.plist"
fingerprint="$(shasum "$APP/macos/Info.plist")"
(cd "$APP" && "$AN" add macos >/dev/null 2>&1)
if [ "$fingerprint" = "$(shasum "$APP/macos/Info.plist")" ]; then
  ok "an add macos repeated: it does not trample the user's plist"
else
  ko "an add macos repeated: it does not trample the user's plist"
fi

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
# The label is written by the scaffold the CLI generates. What is checked is the
# count, which is what proves the tap arrived: before it, the title carries a
# zero.
contains "$output" 'Button#[0-9]+ .*title=Taps: 1' 'a tap made it all the way to the component signal'

# ---------------------------------------------------------------------------
# A password written into the manifest
# ---------------------------------------------------------------------------
#
# It is checked here, in a real project, and not only in `check-signing.sh`:
# what has to be true is that **every** command inside a project refuses, not
# only the ones that sign. `an build` is the one that would otherwise let a
# committed password go unnoticed for months.
edit_manifest() { # $1 python expression over `m`, the parsed manifest
  python3 -c 'import json,sys; p=sys.argv[1]; m=json.load(open(p)); exec(sys.argv[2]); json.dump(m,open(p,"w"),indent=2)' \
    "$APP/angular-native.json" "$1"
}
edit_manifest 'm["signing"]={"android":{"keystore":"release.keystore","storePassword":"hunter2"}}'
output="$(cd "$APP" && must_fail "$AN" build)"
contains "$output" 'signing[.]android[.]storePassword' \
  'a password in the manifest stops `an build`, not only the signing commands'
contains "$output" 'storePasswordEnv' 'and it says to name a variable instead'
edit_manifest 'm.pop("signing",None)'

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

# ---------------------------------------------------------------------------
# The four package managers
# ---------------------------------------------------------------------------
#
# Everything above runs under whichever manager laid out this repository's
# `node_modules`, because that is the tree the project's is cloned from and the
# lockfile the climb in `PackageManager::detect` finds. That covers one of the
# four and says nothing about the other three, and `node_modules` is not a
# format they agree about: pnpm hoists only the direct dependencies and puts the
# rest under `node_modules/.pnpm`, yarn Berry defaults to Plug'n'Play and writes
# no `node_modules` at all, and the three `add` command lines are not
# interchangeable — `yarn add file:x.tgz` is turned down outright for not being
# `package-name@range`.
#
# So each one gets a project of its own and a real install. That is the one
# thing here that needs the network: a manager that cannot download Angular is
# reported as skipped rather than passed, the same as one that is not installed.

# A plugin that depends on another plugin, as two directories the project
# depends on by path. It is the shape that tells the layouts apart: npm, bun and
# yarn hoist the second one to the top of `node_modules`, and pnpm leaves it
# inside the first one's directory in the virtual store, where only Node's own
# climb finds it.
plugin_fixture() { # $1 = the project directory
  local dir="$1" name module
  for name in a b; do
    module="fix$name"
    mkdir -p "$dir/vendor/plugin-$name/native/ios"
    echo "// $module" >"$dir/vendor/plugin-$name/native/ios/${module}Plugin.swift"
  done
  cat >"$dir/vendor/plugin-b/package.json" <<'JSON'
{
  "name": "@fixture/plugin-b",
  "version": "0.0.1",
  "angularNative": {
    "module": "fixb",
    "ios": { "sources": "native/ios", "register": "fixbPlugin" }
  }
}
JSON
  cat >"$dir/vendor/plugin-a/package.json" <<'JSON'
{
  "name": "@fixture/plugin-a",
  "version": "0.0.1",
  "dependencies": { "@fixture/plugin-b": "file:../plugin-b" },
  "angularNative": {
    "module": "fixa",
    "ios": { "sources": "native/ios", "register": "fixaPlugin" }
  }
}
JSON
  node -e '
    const fs = require("fs"), path = process.argv[1] + "/package.json";
    const manifest = JSON.parse(fs.readFileSync(path, "utf8"));
    manifest.dependencies["@fixture/plugin-a"] = "file:./vendor/plugin-a";
    fs.writeFileSync(path, JSON.stringify(manifest, null, 2) + "\n");
  ' "$dir"
}

# Yarn Berry installs into Plug'n'Play unless told otherwise, and PnP is the one
# layout `an` cannot work in: there is no `node_modules`, and `ngc` and esbuild
# are both plain processes reading the disk. What is checked is that it says so
# and leaves the project alone — the failure this replaces was `yarn add` going
# through, writing into the user's lockfile, and the run then ending on a
# message about a missing `node_modules/.bin/ngc`.
check_pnp_refusal() { # $1 = the project directory
  local dir="$1" output before
  before="$(shasum "$dir/package.json")"
  if ! output="$(cd "$dir" && must_fail "$AN" init)"; then
    ko 'yarn: a Plug'"'"'n'"'"'Play project is turned down for being one'
    return
  fi
  contains "$output" "Plug'n'Play" 'yarn: a Plug'"'"'n'"'"'Play project is turned down for being one'
  contains "$output" 'nodeLinker: node-modules' 'yarn: and the refusal says what to put in .yarnrc.yml'
  if [ "$before" = "$(shasum "$dir/package.json")" ]; then
    ok 'yarn: and nothing was installed before it gave up'
  else
    ko 'yarn: and nothing was installed before it gave up'
  fi
}

check_manager() { # $1 = the program, $2… = its install command
  local manager="$1"
  shift
  if ! command -v "$manager" >/dev/null 2>&1; then
    echo "  skipped  $manager is not installed on this machine"
    return
  fi

  local dir="$WORK/pm-$manager" log
  rm -rf "$dir"
  scaffold_project "$dir"
  plugin_fixture "$dir"
  log="$(mktemp)"

  # This project sits under the SDK's own directory, and Yarn Berry climbs until
  # it finds a package.json: it then refuses to install, because a directory
  # inside another project has to be one of its workspaces. An empty lockfile is
  # what its own message asks for — "if you intend it to be a completely
  # separate project, create an empty yarn.lock file in it" — and it is also
  # what makes `PackageManager::detect` say yarn.
  if [ "$manager" = yarn ]; then
    : >"$dir/yarn.lock"
  fi

  # Berry turns installs immutable when it thinks it is in CI, and there is no
  # lockfile here for it to be immutable about.
  if ! (cd "$dir" && YARN_ENABLE_IMMUTABLE_INSTALLS=false "$@" >"$log" 2>&1); then
    echo "  skipped  $manager could not install the project (no network?); see $log"
    return
  fi

  # Yarn Berry, before anything else: with no `node_modules` written there is
  # nothing for the rest of this to check, so it is turned down and the project
  # is told to use the other linker.
  if [ -f "$dir/.pnp.cjs" ]; then
    check_pnp_refusal "$dir"
    printf 'nodeLinker: node-modules\n' >>"$dir/.yarnrc.yml"
    if ! (cd "$dir" && YARN_ENABLE_IMMUTABLE_INSTALLS=false "$@" >"$log" 2>&1); then
      echo "  skipped  $manager could not reinstall with nodeLinker: node-modules; see $log"
      return
    fi
  fi

  if ! (cd "$dir" && "$AN" init >"$log" 2>&1); then
    ko "$manager: an init goes through in a project it laid out"
    tail -20 "$log"
    return
  fi
  ok "$manager: an init goes through in a project it laid out"
  # The two framework packages have to be readable from the project, not merely
  # named in the package.json: under pnpm what is at the top of `node_modules`
  # is a link into the virtual store, and a broken one looks the same until
  # something opens it.
  exists "$dir/node_modules/@angular-native/platform/dist/public-api.js" \
    "$manager: and the framework packages are readable where the build looks for them"
  contains "$(cat "$dir/package.json")" 'file:\.angular-native/vendor/angular-native-platform' \
    "$manager: and the dependency it wrote points at the vendored tarball"

  if (cd "$dir" && "$AN" add ios >"$log" 2>&1) && [ -f "$dir/ios/Info.plist" ]; then
    ok "$manager: an add ios writes the project Info.plist"
  else
    ko "$manager: an add ios writes the project Info.plist"
    tail -20 "$log"
  fi

  # A plugin the app never declared, reached through the one it did. This is the
  # question pnpm's layout raises: with only the direct dependencies hoisted,
  # anything that resolution finds by flattening is not going to be there.
  local plugins
  plugins="$(cd "$dir" && "$AN" plugins 2>&1 || true)"
  contains "$plugins" '^fixa  \(@fixture/plugin-a\)' \
    "$manager: the plugin the app depends on is found"
  contains "$plugins" '^fixb  \(@fixture/plugin-b\)' \
    "$manager: and so is the plugin that plugin depends on, wherever it was put"

  if ! (cd "$dir" && "$AN" build >"$log" 2>&1); then
    ko "$manager: an build produces the bundle"
    tail -20 "$log"
    rm -f "$log"
    return
  fi
  ok "$manager: an build produces the bundle"
  local output
  output="$(cargo run -q -p an-bridge --example headless -- \
    "$dir/.angular-native/build/bundle/main.js" 3 2>&1 || true)"
  contains "$output" 'Text#[0-9]+ .*"MyApp"' \
    "$manager: and the bundle runs, with the app's title on a native Text node"
  rm -f "$log"
}

echo "== the same project under each package manager"
check_manager npm  npm install --no-audit --no-fund
check_manager bun  bun install
check_manager pnpm pnpm install
check_manager yarn yarn install

# ---------------------------------------------------------------------------
# The same project again, with an `an` that came from npm
# ---------------------------------------------------------------------------
#
# Everything above runs the `an` this repository just compiled, with `AN_HOME`
# pointing at this repository. That is the one thing somebody installing the CLI
# does not have, and it hides the whole question: an `an` on the PATH is useless
# without the shells' Swift, the core's Rust and `scripts/bundle.mjs`, and none
# of those is in the executable.
#
# So the packages are really built, really installed into a prefix of their own,
# and the same three commands are run in a project outside this repository with
# `AN_HOME` unset — which is what `sdk_root`'s third rule and `bin/an.mjs` exist
# for. What is being defended is silent: a path left out of `packages/cli`'s
# payload does not fail here, it fails on somebody's machine at `an ios`, twenty
# seconds in, as a `swiftc` with no sources.
echo
echo "== an, installed the way somebody who has never cloned this gets it"

npm_check() {
  local host prefix app log payload manifest output before
  host="$(node -p 'process.platform + "-" + process.arch')"

  log="$(mktemp)"
  if ! node scripts/build-cli.mjs --pack --binary "$host=$AN" >"$log" 2>&1; then
    echo "  skipped  the CLI packages could not be assembled; see $log"
    return
  fi
  ok "the six packages are assembled, with a real executable for $host"
  rm -f "$log"

  local cli native
  cli="$(ls "$ROOT/build/npm-cli/angular-native-cli-"[0-9]*.tgz 2>/dev/null | head -1)"
  native="$(ls "$ROOT/build/npm-cli/angular-native-cli-$host-"*.tgz 2>/dev/null | head -1)"
  if [ ! -f "$cli" ] || [ ! -f "$native" ]; then
    ko 'both tarballs come out of the assembly'
    return
  fi

  # The payload ships **sources**. An archive in there would mean the release
  # had started carrying a compiled core, and that is a decision with a
  # documented answer — see guide/installing — not something to discover from a
  # tarball that grew by three hundred megabytes.
  local carried
  carried="$(tar -tzf "$cli")"
  contains "$carried" 'package/crates/an-core/src/lib\.rs' \
    'the payload carries the core as Rust source, for the user cargo to build'
  contains "$carried" 'package/shells/ios/Sources/' \
    "the payload carries the shells' Swift, which no host can be built without"
  contains "$carried" 'package/packages/runtime/runtime\.js' 'the payload carries the JS prelude'
  contains "$carried" 'package/scripts/bundle\.mjs' 'the payload carries the bundler'
  if grep -qE -e '^package/.*\.(a|so|dylib|rlib)$' <<<"$carried"; then
    ko 'the payload ships no compiled object, only sources'
  else
    ok 'the payload ships no compiled object, only sources'
  fi

  # `os` and `cpu` are the whole reason there are five packages: without them
  # npm would install all five and every machine would download four
  # executables it cannot run.
  manifest="$(tar -xzOf "$native" package/package.json)"
  contains "$manifest" '"os": *\[' "the executable's package is pinned to one os"
  contains "$manifest" '"cpu": *\[' 'and to one cpu, so npm installs only the matching one'

  # A prefix of its own. Nothing here may touch the machine's real global
  # `node_modules`, and a check that did would be a check nobody can run twice.
  prefix="$WORK/npm-prefix"
  rm -rf "$prefix"
  mkdir -p "$prefix"
  log="$(mktemp)"
  if ! npm install -g --prefix "$prefix" --no-audit --no-fund "$cli" "$native" >"$log" 2>&1; then
    echo "  skipped  the tarballs could not be installed (no network?); see $log"
    return
  fi
  ok 'npm install -g puts the two packages in, and nothing else'

  payload="$prefix/lib/node_modules/@angular-native/cli"
  # The four things `validate_sdk` asks for, asked of the installed tree and not
  # of the tarball: `files` can list a directory npm then leaves out.
  local missing=""
  for needed in packages/runtime/runtime.js scripts/bundle.mjs shells crates; do
    [ -e "$payload/$needed" ] || missing="$missing $needed"
  done
  if [ -z "$missing" ]; then
    ok 'the installed payload has the four things `an` refuses an SDK without'
  else
    ko "the installed payload is missing:$missing"
  fi

  # Outside the repository, and with `AN_HOME` unset: this is the only part of
  # this file where neither the monorepo climb nor the variable can answer the
  # question of where the SDK is.
  app="$(mktemp -d)/my-app"
  scaffold_project "$app"
  log="$(mktemp)"
  if ! (cd "$app" && npm install --no-audit --no-fund >"$log" 2>&1); then
    echo "  skipped  the throwaway project could not be installed (no network?); see $log"
    return
  fi

  npm_an() { (cd "$app" && env -u AN_HOME PATH="$prefix/bin:$PATH" an "$@"); }

  output="$(npm_an --version 2>&1 || true)"
  contains "$output" '^an ' 'the `an` on the PATH is the shim, and it runs the binary beside it'

  if ! npm_an init >"$log" 2>&1; then
    ko 'an init goes through with no checkout and no AN_HOME'
    tail -20 "$log"
    return
  fi
  output="$(cat "$log")"
  ok 'an init goes through with no checkout and no AN_HOME'
  contains "$output" 'node_modules/@angular-native/cli' \
    'and the SDK it found is the payload under node_modules'
  exists "$app/angular-native.json" 'an init from npm: writes the manifest'
  exists "$app/node_modules/@angular-native/platform/dist/public-api.js" \
    'an init from npm: compiles and installs the framework packages'

  if npm_an add ios >"$log" 2>&1 && [ -f "$app/ios/Info.plist" ]; then
    ok 'an add ios from npm: writes the project Info.plist'
  else
    ko 'an add ios from npm: writes the project Info.plist'
    tail -20 "$log"
  fi

  if ! npm_an build >"$log" 2>&1; then
    ko 'an build from npm: produces the bundle'
    tail -20 "$log"
    return
  fi
  ok 'an build from npm: produces the bundle'
  output="$(cargo run -q -p an-bridge --example headless -- \
    "$app/.angular-native/build/bundle/main.js" 3 2>&1 || true)"
  contains "$output" 'Text#[0-9]+ .*"MyApp"' \
    "and the bundle runs, with the app's title on a native Text node"

  # A global prefix is very often somewhere the user cannot write, and `cargo`
  # puts `target/` beside the workspace it compiles unless it is told otherwise.
  # Nothing may appear in there.
  if [ -e "$payload/target" ] || [ -e "$payload/build" ]; then
    ko 'nothing is written into the installed package'
  else
    ok 'nothing is written into the installed package'
  fi

  # And the executable on its own, with no shim to hand it `AN_HOME`. This is
  # `sdk_root`'s third rule: the climb from the executable finds
  # `@angular-native/cli` next door, which is the layout npm, bun and yarn all
  # write.
  output="$(cd "$app" && env -u AN_HOME "$prefix/lib/node_modules/@angular-native/cli-$host/bin/an" \
    plugins 2>&1 || true)"
  contains "$output" 'depends on no plugin|^[a-z]' \
    'the executable run directly, with no shim, still finds the SDK beside it'

  rm -rf "$(dirname "$app")"
  rm -f "$log"
}

if ! command -v node >/dev/null 2>&1; then
  echo "  skipped  node is not installed on this machine"
else
  npm_check
fi

if [ "$fail" -ne 0 ]; then
  exit 1
fi
