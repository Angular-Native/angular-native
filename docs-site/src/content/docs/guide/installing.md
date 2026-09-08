---
title: Installing the CLI
description: "`npm install -g @angular-native/cli` puts `an` on the PATH with the SDK it needs — what the package carries, what your machine still has to have, and why the Rust core is not shipped compiled."
sidebar:
  order: 2
---

```bash
npm install -g @angular-native/cli
an --version
```

That is the whole installation. There is nothing to clone, no `AN_HOME` to set
and no path to point at a checkout: the package carries both the `an` executable
and the SDK it reads from.

## What lands on your disk

Six packages are published and npm downloads two of them.

**`@angular-native/cli`** is the payload. It is the same bytes on every machine,
because what it holds is *source*: the Swift of the five Apple shells, the Java
of the two Android ones, the Rust of the core, `scripts/bundle.mjs`, and the
TypeScript of `@angular-native/primitives` and `@angular-native/platform`. About
7 MB compressed, 17 MB unpacked, most of it the Material Symbols font the
Android shell needs.

**`@angular-native/cli-<host>`** is one native executable and nothing else.
There are five —`darwin-arm64`, `darwin-x64`, `linux-arm64`, `linux-x64`,
`win32-x64`— listed as `optionalDependencies` with `os` and `cpu` set, so npm
installs the one that matches this machine and skips the other four. It is the
arrangement esbuild and swc use, and it is the reason the install is one
executable rather than five.

The `an` on your PATH is a small Node shim in the first package. It asks Node's
resolver where the second one went, tells the executable where the payload is,
and gets out of the way. That indirection is not decoration: under pnpm the two
packages are unrelated directories inside `node_modules/.pnpm` with no path from
one to the other, and Node's resolver is the only thing that knows the answer.

## What your machine still needs

| To build | You need |
|---|---|
| the bundle — `an init`, `an build`, `an plugins` | Node 20.19 or newer, and nothing else |
| iOS, tvOS, visionOS, watchOS, macOS | Xcode, and [Rust](https://rustup.rs) |
| Android, Wear OS | the Android SDK with an NDK, a JDK, and Rust |
| tvOS, visionOS, watchOS | additionally `rustup toolchain install nightly --component rust-src` |

On Linux and Windows the CLI is real but the reachable half is Android: the
Apple platforms need `xcrun`, `swiftc` and the simulators, and those exist only
on macOS.

Android additionally wants Material and the libraries it drags along, resolved
once. The two scripts that do it travel in the package, and they write their
answer next to themselves:

```bash
cd "$(dirname "$(dirname "$(readlink -f "$(which an)")")")"
python3 scripts/fetch-android-deps.py
python3 scripts/prepare-android-deps.py
```

That needs the installed package to be writable, which a global prefix under
`/usr/local` is not. Install the CLI into a prefix of your own —`npm config set
prefix ~/.npm-global`— or keep a checkout and point `AN_HOME` at it.

The first platform command compiles the Rust core, which takes a few minutes
once and is cached afterwards. `rust-toolchain.toml` travels inside the package,
so rustup installs the toolchain and the four targets without being asked.
`cargo` puts what it builds in your project's `.angular-native/build/target`,
never inside `node_modules` — a global install is very often somewhere you
cannot write, and `npm ci` deletes a local one.

## Why the core is not shipped compiled

The obvious next step from "npm installs the executable" is "npm installs the
compiled core too, and then nobody needs Rust". It was weighed and turned down,
and the reason is worth writing out because it looks like an oversight.

The executable is **per host**: five combinations, and each machine downloads
one. The static libraries are **per target**, and one machine needs all of them
— a Mac builds for `aarch64-apple-ios`, `aarch64-apple-ios-sim`, two tvOS
triples, two visionOS, two watchOS, two macOS and four Android ABIs. That is
fourteen archives, not five, and none of them is optional in the way an
executable for another operating system is. Stripped, one of them is around
40 MB, 9 MB compressed; the set comes to hundreds of megabytes per install, and
that is before the second copy the debug and release profiles would each need.

Set against that, what it would save: `rustup`, one command and about 300 MB.
And a machine that is going to run `an ios` already has Xcode on it, and one
that is going to run `an android` already has the NDK — both of which are
several gigabytes. Rust is not the outlier the size makes it look like; it is
the smallest of the three toolchains the command already requires.

There is a second reason, and it is the one that settles it. A prebuilt archive
has to agree with `crates/an-ios/include`'s headers and with the protocol bytes
the shells and the bundle exchange. When they drift, nothing raises: the app
mounts a tree with a hole in it. Every list in this project that is duplicated
across languages has a check script guarding it, and a binary blob compiled
somewhere else is a duplicate nothing can guard.

The three tier-3 Apple targets — tvOS, visionOS, watchOS — are the one place
where prebuilt libraries would genuinely help, because they are the reason those
platforms ask for nightly. If that changes, it will change for those three
alone, and they will still be built from source by anybody who prefers it.

## The three places `an` looks for the SDK

In this order:

1. **`AN_HOME`**, if it is set. The npm shim sets it to its own package before
   running the executable, and anything you set yourself wins over that — which
   is how the framework itself gets worked on with an `an` that came from npm:

   ```bash
   AN_HOME=~/src/angular-native an build
   ```

2. **The path the binary was compiled from**, which is what
   `cargo install --path crates/an-cli` bakes in. An `an` built from a checkout
   finds its way back to it.

3. **Beside the executable.** A climb from the binary looking for
   `@angular-native/cli`, which is what makes it work when it is run directly
   instead of through the shim.

If the place exists but is missing `packages/runtime/runtime.js`,
`scripts/bundle.mjs`, `shells` or `crates`, `an` says which one rather than
failing later inside `swiftc`.

## Without npm

The CLI can be built from source on any host Rust supports, including ones there
is no published executable for:

```bash
git clone https://github.com/Angular-Native/angular-native
cd angular-native
cargo install --path crates/an-cli
```

That `an` finds the SDK by rule 2 above, so the checkout has to stay where it
is. Move it and set `AN_HOME`.

## Upgrading and removing

```bash
npm install -g @angular-native/cli@latest
npm uninstall -g @angular-native/cli
```

The framework packages inside a project are not upgraded by that. They are
vendored as tarballs in `.angular-native/vendor` and committed with the project
on purpose — see
[Taking an Angular project to mobile](/guide/existing-angular-project/). To move
a project to a new release, run `an init --force`, which recompiles and re-packs
them from the SDK now on the PATH.
