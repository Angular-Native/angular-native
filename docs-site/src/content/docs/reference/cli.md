---
title: The `an` CLI
description: Every command, every flag and every default, read from the code — and the handful of asymmetries between them that are worth knowing before you hit one.
sidebar:
  label: The an CLI
  order: 3
---

`an` is the whole toolchain. There is no `.xcodeproj`, no Gradle and no
generated project to keep in sync: it drives `ngc`, esbuild, `cargo`, `swiftc`,
`aapt2`, `javac`, `d8` and `apksigner` itself, and the only file it leaves in
your repository is a small manifest.

```bash
cargo install --path crates/an-cli
```

There are **thirteen commands** and no nested subcommands. Every option is
long-form — there are no short flags anywhere in the CLI.

| Command | What it does |
|---|---|
| `an init [DIR]` | Sets an Angular project up. |
| `an add <PLATFORM>` | Creates the project's platform directory. |
| `an build [APP]` | Compiles and bundles the JS, and stops there. |
| `an ios [APP]` | Builds and runs on the iOS simulator. |
| `an tvos [APP]` | The Apple TV simulator. |
| `an visionos [APP]` | The Vision Pro simulator. |
| `an watchos [APP]` | The Apple Watch simulator. |
| `an macos [APP]` | Builds a `.app` and opens it on this machine. |
| `an android [APP]` | Builds an APK and installs it on a connected phone. |
| `an wearos [APP]` | The same, for a device shaped like a watch. |
| `an env <PLATFORM>` | Prints the environment a cross-compilation needs. |
| `an plugins [APP]` | Lists the plugins an app pulls in, and optionally checks one platform. |
| `an dev [APP]` | Serves the bundle, watches, and hot-refreshes the running app. |

## `an init`

```bash
an init [DIR] [--name <NAME>] [--id <ID>] [--force]
```

| Argument | Default |
|---|---|
| `DIR` | the current directory |
| `--name` | the previous `app.name`, else the PascalCase of `package.json`'s `name` |
| `--id` | the previous `app.bundleId`, else `dev.angularnative.<slug>` |
| `--force` | off — rewrites only what `an` generates, never your code |

It refuses to run unless there is an `angular.json` **and** `@angular/core` in
the dependencies. The bundle id must be at least two dot-separated segments,
each starting with an ASCII letter and containing only ASCII alphanumerics: no
hyphens and no underscores.

What it leaves behind:

| Path | What it is |
|---|---|
| `angular-native.json` | The manifest: name, bundle id, entry, platforms. Always rewritten. |
| `.angular-native/tsconfig.json` | The native build's tsconfig — full AOT, strict templates, `types: []`. `--force` rewrites this. |
| `src/main.native.ts` | The native entry point. **Never** rewritten, `--force` or not. |
| `src/app/app-native.ts` | A root component to start from. Never rewritten either. |
| `.angular-native/vendor/*.tgz` | The two framework packages, compiled with *your* `ngc` in partial mode and packed. Meant to be committed. |
| `.gitignore` | One line appended, `/.angular-native/build/`, and only if it is not already there. |
| `package.json` | The two vendored packages installed by path; `@angular/compiler-cli` added as a dev dependency if no `ngc` is reachable. |

Everything that can fail runs before the manifest is written, so a failed `init`
leaves the project uninitialised rather than half-initialised. Anything that
already exists is left alone and reported as such.

**It touches neither `angular.json` nor `src/main.ts` nor the web
`tsconfig.json`.** `ng build` and `ng serve` keep working exactly as before.

## `an add`

```bash
an add <PLATFORM>
```

The platform is required, and there are exactly five: **`ios`, `tvos`,
`visionos`, `macos`, `android`**. There is no `an add watchos` and no
`an add wearos`.

Each creates a directory with exactly one file in it — `ios/Info.plist`,
`tvos/Info.plist`, `visionos/Info.plist`, `macos/Info.plist`,
`android/AndroidManifest.xml` — derived from the shell's, with the name and the
bundle id substituted in. If the file is already there it is left alone; either
way the platform is recorded in `angular-native.json`.

`macos` decorates both the way `tvos` and `visionos` do: an app called `MyApp`
becomes `MyAppMac` with `.mac` on the end of its identifier. One project builds
for the phone and for the desktop from the same manifest, and two bundles
sharing an identifier are one app as far as the system is concerned — one
container, one set of defaults, one keychain partition.

Two things worth knowing:

- `an add` only works **inside a project**. Run in the monorepo it errors.
- A project targeting Wear OS needs `android/AndroidManifest.wear.xml`, and
  `an add` never creates it: write it by hand, or let the build fall back to the
  shell's. `an add android` is what creates the directory it goes in.

## `app.appearance`

One word in `angular-native.json`, and every platform is told in its own:

```json
"app": { "name": "MyApp", "bundleId": "com.example.myapp", "appearance": "system" }
```

| | |
|---|---|
| `system` | Follow the device. Light phone, light app. **The default.** |
| `light` | Always light, whatever the system says. |
| `dark` | Always dark. |

| Platform | How it is told |
|---|---|
| Android, Wear OS | `<meta-data>` in the merged manifest; the Activity turns it into `setDefaultNightMode` before `super.onCreate`. |
| iOS, tvOS, visionOS | `UIUserInterfaceStyle` in the `Info.plist` — `system` is the key's **absence**, which is what UIKit already does. |
| macOS | The same plist key, read back by the shell into `NSApp.appearance`, because AppKit does not act on a UIKit key by itself. |
| watchOS | Ignored. A watch has no light mode worth having: the screen is OLED and what is not painted draws no power. |

A word that is none of the three stops the build and names the three that are
not — a typo that silently meant `system` would be a setting that does nothing
for a reason nobody can see.

What it does **not** do is paint the screen. That is the app's own background,
and the appearance is what the system's own controls follow: dialogs, date
pickers, selection handles. An app that follows the device has to paint with
the device too.

## `an env`

```bash
$(an env android) cargo build --target aarch64-linux-android -p an-android
```

Nobody needs this for an ordinary build: `an android` sets the same things on
the `cargo` it spawns. It exists because those settings stopped being a
committed `.cargo/config.toml` — they hold this machine's NDK path, its
version and this host's name, none of which belongs in a file everybody clones
— and something still has to hand them to a `cargo` that is not `an`'s: a
check script, a CI job, an editor cross-checking.

It prints an **`env` prefix** rather than a list of `export`s, and that is not
a style choice. Most of the names carry the target triple with its hyphens, and
a shell cannot export one of those — `export CC_aarch64-linux-android=…` is
`not a valid identifier`. Nor can the hyphens be swapped for underscores to
suit it: `cc` would take either, and **bindgen reads only the hyphenated
form**, so a build that looked fixed would go back to asking Apple's clang to
compile for Android and failing to find `stdio.h`. `env` takes `NAME=VALUE` as
arguments and has no opinion about identifiers.

## The build-and-run commands

They all share the same shape.

| Command | `APP` default | `--device` default | `--release` | `--no-launch` |
|---|---|---|:--:|:--:|
| `an build` | `examples/hello-angular` | — | ✓ | — |
| `an ios` | `examples/hello-angular` | `iPhone 17 Pro` | ✓ | ✓ |
| `an tvos` | `examples/hello-tv` | `Apple TV 4K (3rd generation)` | ✓ | ✓ |
| `an visionos` | `examples/hello-vision` | `Apple Vision Pro` | ✓ | ✓ |
| `an watchos` | `examples/hello-watch` | `Apple Watch Series 11 (46mm)` | ✓ | **—** |
| `an macos` | `examples/controls` | **—** | ✓ | ✓ |
| `an android` | `examples/hello-angular` | **—** | ✓ | ✓ |
| `an wearos` | `examples/hello-wear` | an `adb` serial, no default | ✓ | ✓ |

And the flags that produce something for a device or a store. They are on the
platform's own subcommand rather than in a command of their own, because they
change what that build *is*, not what happens afterwards:

| Command | Flag | What comes out |
|---|---|---|
| `an ios` | `--physical` | Signed for a connected iPhone or iPad, installed with `devicectl`. `--device` then names the device rather than a simulator. |
| `an ios` | `--archive` | `<Name>.xcarchive` and `<Name>.ipa`. Implies `--release`. Conflicts with `--physical`. |
| `an android`, `an wearos` | `--sign` | Signed with the release keystore instead of the debug one. |
| `an android`, `an wearos` | `--aab` | An Android App Bundle. Implies `--sign`; never installs. |
| `an android`, `an wearos` | `--abi` | The ABIs inside it: `arm64-v8a`, `x86_64`, `armeabi-v7a`, repeated or comma-separated. Replaces the default rather than adding to it. |
| `an macos` | `--sign` | Developer ID signature with the hardened runtime, instead of ad hoc. |
| `an macos` | `--notarize` | That, submitted to Apple, waited on and stapled. Implies `--sign`. |
| `an macos` | `--dmg` | A `.dmg`, signed and notarised in its own right when those are on. |

All of them need settings that live in `angular-native.json`, and every one of
them fails before anything is compiled when a credential is missing. See
[Signing and distribution](/guide/signing-and-distribution/), which is also the
page that says what to get from Apple and Google.

`--release` and `--sign` are separate on purpose: the first is about the
compiler, the second about the key. A build for Google Play wants both.

`--abi` defaults to different things for the two artefacts. An APK gets
`arm64-v8a` alone, because every extra ABI is another cross-compilation of the
core and the dev loop pays it on every save. A bundle gets `arm64-v8a` and
`x86_64`, because Play splits a bundle by ABI — nobody downloads the slice they
cannot run, and a bundle without `x86_64` is one the store does not offer to a
Chromebook or an emulator. `armeabi-v7a` is in neither: it is opt-in.
[Android · Which ABIs](/platforms/android/#which-abis) says why.

Four asymmetries in the table above are real and not typos:

- **`an watchos` has no `--no-launch`.** It is the only build command without
  one.
- **`an macos` has no `--device`** because there is no simulator: the `.app`
  runs on the machine that compiled it.
- **`an android` has no `--device` either.** The device is picked by shape.
  `an wearos` does take one, because a watch is identified by an `adb` serial
  rather than by a simulator name.
- **`an build` stops after the bundle** and prints its path on standard output.

`--no-launch` builds the artefact, prints where it landed, and returns.

### Picking a device

On Apple platforms `--device` is a simulator name. The lookup parses
`xcrun simctl list devices available -j` as JSON — deliberately, because
`simctl` prints the udid before the name and grepping would return the
neighbour's — filters runtimes by family, prefers a booted one, and otherwise
takes the first match. If no runtime of that family exists at all, it says so
and gives you the download command rather than claiming it cannot find a device.

On Android the device is picked by asking `ro.build.characteristics`: `watch`
selects the Wear device, anything else the phone. With one device of the right
shape it is used; with none or several you are asked to pass `--device`, and a
`--device` of the wrong shape is refused. That last check matters: a watch APK
installs on a phone without complaint, and starts, and paints, and the only
thing it does not do is be a watch app.

### Toolchain per platform

| Platform | Rust target | Crate | Toolchain |
|---|---|---|---|
| iOS · iPadOS | `aarch64-apple-ios-sim` | `an-ios` | stable |
| tvOS | `aarch64-apple-tvos-sim` | `an-ios` | **nightly + `rust-src`** |
| visionOS | `aarch64-apple-visionos-sim` | `an-ios` | **nightly + `rust-src`** |
| watchOS | `aarch64-apple-watchos-sim` | `an-watch` | **nightly + `rust-src`** |
| macOS | `aarch64-apple-darwin` | `an-macos` | stable |
| Android · Wear OS | `aarch64-linux-android` | `an-android` | stable |

The three nightly targets are tier 3 and ship no prebuilt `std`, so it is built
on the spot with `-Z build-std=std,panic_abort`. A cargo failure on one of them
is re-reported with the two `rustup` commands you need, rather than as a wall of
build-std output.

Everything Apple goes through `xcrun swiftc` in a single invocation over
`shells/<platform>/Sources` plus `shells/shared`, sorted for reproducibility.
There is no `.xcodeproj` anywhere. macOS names `WebKit`, `MapKit`,
`AVFoundation` and `AVKit` explicitly on the link line, because a Rust
`staticlib` does not carry its framework dependencies through — without them the
app builds, signs, launches and then dies on the first view of that class.

Android has no Gradle: `aapt2 compile` and `link`, `javac`, `d8`, `zip`,
`zipalign` and `apksigner`, with a debug keystore created on first use. The
Material 3 dependencies are resolved once by
`python3 scripts/fetch-android-deps.py` and
`python3 scripts/prepare-android-deps.py`.

### Where things land

The build root is `build/` inside the monorepo and `<project>/.angular-native/build/`
in your own project.

```text
build/js/…                        ngc output
build/bundle/…/main.js            the bundle
build/ios/<Name>.app
build/tvos/<Name>TV.app
build/visionos/<Name>Vision.app
build/watchos/AngularNativeWatch.app
build/macos/AngularNativeMac.app
build/android/<AppName>.apk       and <AppName>-wear.apk for Wear
build/ios/<Name>.xcarchive        --archive
build/ios/<Name>.ipa              --archive
build/macos/<Name>.dmg            --dmg
build/android/<AppName>-release.apk   --sign
build/android/<AppName>-release.aab   --aab
```

The release artefacts are named apart from the debug ones on purpose: a signed
build quietly overwriting the APK your emulator has been running is how the
wrong file reaches a store.

tvOS and visionOS get a name and a bundle-id suffix — `TV`/`.tv` and
`Vision`/`.vision` — so that building one does not overwrite the other's `.app`
and installing one does not uninstall the other.

## `an plugins`

```bash
an plugins [APP] [--platform ios|android]
```

Without `--platform` it lists what the app pulls in, one line per plugin: module
name, package, which platforms it covers, and where it lives. With
`--platform` it runs the same coverage check the build runs, so it exits
non-zero exactly when the build would refuse.

`--platform` takes only `ios` and `android`. See
[Plugins](/extending/plugins/) for what coverage means and why a missing
platform is a hard error.

## `an dev`

```bash
an dev [APP] [--port <N>] [--device <NAME>]
       [--android] [--wearos] [--watchos] [--tvos] [--visionos] [--macos]
       [--no-launch]
```

Serves the bundle on `127.0.0.1:8420` — `--port` changes it — and watches for
changes. The port is bound *before* anything is built, so a second `an dev`
fails immediately instead of after a two-minute build.

The app is given the server's URL at build time, and it is `127.0.0.1` for
every target. On the Apple ones that is true on its own: the watch shares the
Mac's network and a Mac app is not inside anything at all. On Android it is made
true — `adb reverse` opens the port on the device pointing back here, which
works on an emulator and on a phone over USB alike, and replaced the `10.0.2.2`
that only ever meant anything inside an emulator.

`--android` wins over `--wearos`, which wins over `--watchos`, `--tvos`,
`--visionos`, `--macos`, and iOS is what you get with none of them. They are not
declared as mutually exclusive, so `an dev --android --tvos` quietly builds
Android.

There is no `an dev --ios` — iOS is the default.

`--macos` has no simulator to launch into: `an dev --macos` kills the window
that was already open and opens a new one, the same as `an macos`.

`--device` behaves as "if you did not change it, use this platform's default":
`--watchos` gets the watch, `--tvos` the Apple TV, `--visionos` the headset.
`--android` and `--macos` ignore it entirely — the Mac has no device to name.
`an dev --wearos` treats it as an `adb` serial.

`[APP]` defaults to `examples/hello-angular`, except with `--wearos`
(`examples/hello-wear`) and `--macos` (`examples/controls`), which is the same
default each platform's own subcommand uses and for the same reason: a phone
screen is not readable on a round 227-point face, and on the desktop `controls`
is what shows at a glance what AppKit draws.

It watches `<app>/src` always, and `packages/` as well when run inside the
monorepo. Only `.ts`, `.js`, `.html` and `.json` count, debounced 250 ms. A
compile that fails prints the error and keeps watching — it never takes the
server down.

### What a save actually does

The dev bundle is two halves in one file. Everything resolved as a bare package
— Angular, rxjs, `@angular-native/*` — goes in the top half and is stamped with
a hash; your own relative modules go in the bottom half and are re-evaluated on
top.

- If the **top half changed**, the bottom half is not evaluated at all and the
  app restarts. Only one copy of Angular fits in the interpreter, and the
  running one cannot be replaced.
- Otherwise Angular's metadata is swapped in place, keeping the same class
  objects, so instances and signals survive and you stay on the same screen.
  This is refused — and a restart forced — when the set of components changed
  size, when a key went missing, or when a class changed between component and
  directive.
- A thrown exception anywhere in that path also forces a restart, rather than
  leaving you looking at half-updated code.

On a cold restart the native views are torn down, the JS engine is new, the tree
is empty and every signal is back at its initial value. Two things survive a hot
reload on purpose: the hot-state signals, and the router's history.

The restart is `an dev`'s own, not yours: the engine is thrown away and stood
up again inside the running process, and the app is back on screen without
anybody touching the terminal. What a restart costs is component state, not the
dev loop.

`packages/runtime/runtime.js` is the exception to all of the above, and it is
not in either half. It is `include_str!`-ed into the host crate and evaluated
when the engine starts, so it travels in the **native binary**: no bundle can
carry it and no reload can reach it. Saving it rebuilds the app and puts it
back on the device — which is the one thing here that costs a compile.

Apple shells talk to the server over a WebSocket; the Android shell long-polls,
because there is no platform WebSocket there.

## Environment

| Variable | Read by | What it does |
|---|---|---|
| `AN_HOME` | `an` | Where the framework's sources are, when running outside the monorepo. Checked first, and **validated** — pointing it somewhere that is not an SDK is an error, not a fallback. |
| `CARGO_TARGET_DIR` | `an` | Where to look for the compiled staticlibs. |
| `ANDROID_HOME`, `ANDROID_SDK_ROOT` | `an` | The Android SDK, in that order, falling back to `~/Library/Android/sdk`. |
| `AN_IOS_TEAM`, `AN_IOS_IDENTITY`, `AN_IOS_PROFILE` | `an` | Override `signing.ios.*`. |
| `AN_MACOS_IDENTITY`, `AN_MACOS_NOTARY_PROFILE` | `an` | Override `signing.macos.*`. |
| `AN_ANDROID_KEYSTORE`, `AN_ANDROID_KEY_ALIAS` | `an` | Override `signing.android.*`. |
| `AN_ANDROID_KEYSTORE_PASSWORD`, `AN_ANDROID_KEY_PASSWORD` | `an` | The two passwords. They exist **only** here — the manifest names the variable, never the value. |
| `AN_BUNDLETOOL` | `an` | Where `bundletool.jar` is, for `--aab`. |

The signing variables win over `angular-native.json`, which is what CI wants and
what makes those paths usable from inside this repository, which has no project
manifest.

Outside the monorepo, `an` walks up from the working directory looking for an
`angular-native.json`. The SDK itself is `AN_HOME` if set, otherwise the path
baked in at compile time — which is what makes `cargo install --path crates/an-cli`
work from anywhere. Either way the root is validated to contain the runtime, the
bundler, the shells and the crates. If nothing is found but the directory *is* an
Angular project, it says to run `an init` there rather than reporting no root.

## What stops a build

Each of these is a hard error with the reason in it, not a warning:

- **A plugin that does not cover the platform being built.** Checked before
  anything compiles. See [Plugins](/extending/plugins/).
- **Any plugin at all on watchOS or macOS.** Neither host has a plugin registry,
  so the build refuses rather than shipping an app whose every call would be
  rejected at runtime.
- **Kotlin in a plugin with no Kotlin compiler vendored.** `an` runs `kotlinc`
  itself — there is no Gradle to do it — and it names the `.kt` it cannot
  compile rather than signing an APK without it. `python3
  scripts/fetch-android-deps.py` fetches the compiler once; an app whose plugins
  are all Java never needs it.
- **Two plugins claiming the same module name**, or asking for the same
  `Info.plist` key, entitlement or `uses-feature` with different values.
- **An `Info.plist` that disagrees with `angular-native.json`.** Checked before
  cargo and swiftc, and it names the `an add` you are missing.
- **An `AndroidManifest.xml` that no longer declares `package="dev.angularnative"`.**
- **An empty Android dependency cache**, pointing at
  `scripts/prepare-android-deps.py`.
- **A password written into `angular-native.json`.** Refused by name, by every
  command that runs inside a project — not only the ones that sign.
- **A keystore or a profile that git can see**, whether committed or merely not
  ignored.
- **Any missing or wrong signing credential**: no section for the platform, a
  profile that is absent, unreadable, expired or for another app, a team that
  disagrees with the profile, a certificate that is not in the keychain, a
  keystore whose password or alias is wrong, a `bundletool` that is not there.
  All of them before `cargo` is called.

## Two things to know before you hit them

**Outside the monorepo, six invocations fail as written.** `an macos`, `an tvos`,
`an visionos`, `an watchos`, `an wearos` and `an dev --wearos` always pass their
default example along, and outside the monorepo an explicit app path that is not
the project root is an error. Pass the project explicitly:

```bash
an tvos .
an macos .
```

`an build`, `an ios`, `an android`, `an plugins` and plain `an dev` pass no
default and are unaffected.

**`an dev --watchos` defaults to the phone example.** `an watchos` defaults to
`examples/hello-watch` because `hello-angular` is unreadable at 205 points;
`an dev --watchos` does not carry that default over. Name the example.
