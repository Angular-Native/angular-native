---
title: Taking an Angular project to mobile
description: You already have an `ng new` app and want it running on native views. Nothing has to move into this repository.
sidebar:
  order: 3
---

This page is for someone who already has an Angular app — one from `ng new` —
and wants it running on native views. Nothing has to be cloned inside this
repository and the project does not have to move to `examples/`: `an` works
just as well from outside.

## End to end

```bash
# 1. The binary, once per machine. It leaves `an` on the PATH and remembers
#    where the SDK is, so from here on you never come back to this directory.
cd angular-native
cargo install --path crates/an-cli

# 2. Any Angular project.
npx @angular/cli@latest new my-app
cd my-app

# 3. Set it up.
an init

# 4. The platform.
an add ios

# 5. To the simulator.
an ios
```

`an dev` instead of `an ios` also brings up the dev server and reloads the app
on save, with the screen's state left intact.

## What `an init` leaves in the project

```text
my-app/
  angular-native.json           the manifest: name, identifier and entry point
  .angular-native/
    tsconfig.json               the tsconfig for the native build
    vendor/*.tgz                the framework's two packages, packed
    build/                      the .app, the APK and the compiled JS  (gitignored)
  ios/Info.plist                written by `an add ios`; yours from then on
  android/AndroidManifest.xml   written by `an add android`; likewise
  src/main.native.ts            the native app's entry point
  src/app/app-native.ts         its root component
```

And two new dependencies in `package.json`:

```json
"@angular-native/platform": "file:.angular-native/vendor/angular-native-platform-0.0.1.tgz",
"@angular-native/primitives": "file:.angular-native/vendor/angular-native-primitives-0.0.1.tgz"
```

Nothing else. The web app stays exactly as it was: `ng build` and `ng serve`
still work, because `an` touches neither `src/main.ts`, nor `angular.json`, nor
the project's `tsconfig.json`.

## Two roots, two component trees

The native app starts from `src/main.native.ts` and the web one from
`src/main.ts`. Services, models, signals and state are shared without ceremony —
they are ordinary TypeScript. **Templates are not.** A `<div>` has no native
equivalent, and `<an-view>` cannot be painted in a browser. Sharing templates
would need an intermediate language that translated into both, which is exactly
what this project decided not to do: here an `<an-switch>` *is* a `UISwitch`,
looking and behaving however it does in that version of iOS, and you cannot
promise that and paint it in HTML at the same time.

So the split is:

| Shared | Not shared |
|---|---|
| services, models, signals, validation, HTTP client | templates, CSS, anything that depends on the DOM |

## The three decisions

### Where `@angular-native/platform` and `primitives` come from

Both packages belong to this repository and are not published to npm yet. There
were three ways to get them into an outside project:

- **A local path**, `file:../angular-native/packages/primitives`. The easiest to
  write and the worst of the three: npm installs it as a symlink, the path is
  the one on the disk of whoever ran `an init`, and the committed
  `package-lock.json` is useless to anyone else on the team.
- **A vendored tarball**: `npm pack` inside the project, with its version, and
  the dependency pointing at it. It is an exact copy of what the SDK held that
  day. It is committed with the project, `npm ci` reinstalls it with no network
  and no SDK present, and the build cannot drift from the framework without
  somebody seeing it in a diff.
- **Publishing them to npm**, which is what will be needed the day this leaves
  the drawer.

The second one was chosen. And the day they are published only the specifier
changes: what lands in `node_modules` is exactly the same, so nothing above it —
not the tsconfig, not esbuild, not the bundle — notices.

What gets packed is **not the sources**: it is the two compiled packages, with
their `.d.ts` and in partial mode, which is how any Angular library is
published. There is a trap here that took a while to find: TypeScript **emits no
JavaScript for sources under `node_modules`**, it takes them for an already
compiled external library. Installing the `.ts` files and pointing the tsconfig
at them with `paths` compiles without a single complaint and produces a bundle
missing half the framework, which esbuild discovers three steps later. Compiling
them before packing turns that silence into the boring case: they resolve like
any other dependency and the Angular Linker in `scripts/bundle.mjs` does the
rest.

They are compiled with the **project's** `ngc`, not the SDK's, so that the
`.d.ts` files and the partial declarations come out of the same Angular version
the app is compiled against.

### What `an init` does if the project is not Angular, or is already set up

Neither can end in a half-made mess, and each is solved differently.

**If it is not Angular**, it does not start. `an init` looks for `angular.json`
and `@angular/core` in `package.json` before touching anything, and if either is
missing it names it and explains that this runs inside an already created
project. There is no intermediate state because nothing was written.

**If it is already set up**, it carries on and only adds what is missing. A file
that already exists is left alone and said to have been left alone; `--force`
rewrites what we generate — the tsconfig, and reinstalls the packages — but
never the app's code. Running `an init` twice is safe by definition, and running
it after updating the SDK is how you pull in the new packages.

What holds both of these up is the order: **`angular-native.json` is written
last**. Everything that can fail — npm, `ngc`, a half-installed SDK — happens
before. If something goes wrong the manifest never comes into existence, `an`
still says the project is not set up, and trying again is safe. There is no
"half initialised" state for anyone to clean up by hand.

### Where the native projects live, and what happens if you edit them

Capacitor creates `ios/` and `android/` as complete Xcode and Gradle projects,
declares them the user's property, and `npx cap sync` only copies the web assets
back and registers the plugins. It is the right answer *for Capacitor*: without
the `.xcodeproj` there is no build, so the project has to exist and somebody has
to own it. The price is a file of thousands of lines nobody reviews in a diff
and that drifts out of sync in silence.

Here there is no `.xcodeproj` and no Gradle — `an` invokes `swiftc`, `aapt2`,
`d8` and `apksigner` directly — so no native project needs to exist at all. The
answer is to split in two what Capacitor keeps together:

- **Configuration: yours, and committed.** `ios/Info.plist` and
  `android/AndroidManifest.xml`. `an add` creates them, and from that moment
  `an` copies them into the build and **never rewrites them**. That is where
  permissions, orientations, privacy keys and whatever else the app declares go.
  They are fifty-line files you can read in a diff.
- **The `.app` and the APK: build output.** They live in
  `.angular-native/build/`, they are in `.gitignore` and they are remade whole on
  every compile. Editing them there makes no sense and there is no question of
  what happens if somebody does: they are deleted on the next `an ios`.

There is one way left for the two halves to drift apart, and it is covered: if
you change `app.name` or `app.bundleId` in `angular-native.json` and do not
touch the plist, iOS installs an app that looks for an executable that is not
there and vanishes when you open it, without a single error. `an ios` compares
the two **before compiling anything** and stops saying which is which. Same for
the `package` in `AndroidManifest.xml`, which has to stay the shell's classes':
the application identifier is set by `aapt2` with `--rename-manifest-package`.

## Where `an` looks for the SDK

Outside the monorepo, `an` needs to know where the crates and the shells are.
Two places, in this order:

1. `AN_HOME`, if it is set.
2. The path it was built from. `cargo install --path crates/an-cli` records it
   inside the binary, so an `an` on the PATH knows how to find its way back to
   its repository.

If the place exists but something is missing, it says what. That is why
`angular-native.json` does **not** store the SDK path: it would be the path on
the disk of whoever ran `an init`, and the file is committed.

## Checking it without a simulator

```bash
./scripts/check-external.sh
```

It builds a real Angular project in `build/check-external/`, puts it through
`an init`, `an add ios`, `an add android` and `an build`, and runs the resulting
bundle in the headless renderer. It also covers the three ways this has of
ruining somebody else's project: `an init` on something that is not Angular, a
repeated `an init` that would overwrite hand-written code, and an `Info.plist`
out of sync with the manifest. It runs inside `./scripts/check-all.sh`.
