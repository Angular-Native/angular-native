# @angular-native/cli

`an` — the command that takes an ordinary Angular project to iOS, Android,
macOS, tvOS, visionOS, watchOS and Wear OS, on real native views. No WebView, no
DOM, no zone.js.

```bash
npm install -g @angular-native/cli

cd my-app        # anything out of `ng new`
an init          # dependencies, tsconfig and native entry point
an add ios       # writes ios/Info.plist, yours from then on
an build         # the bundle: ngc + esbuild
an ios           # the .app, and the simulator
```

`an init` touches neither `src/main.ts` nor `angular.json`, so `ng build` and
`ng serve` keep working exactly as before.

## What gets installed

Six packages, of which npm downloads two. `@angular-native/cli` is this one: the
`an` shim and the SDK payload — the Swift and Java sources of the seven native
shells, the Rust sources of the core, `scripts/bundle.mjs`, and the framework's
TypeScript. Beside it, one of `@angular-native/cli-{darwin-arm64, darwin-x64,
linux-arm64, linux-x64, win32-x64}` carries the single native executable for
this machine; the other four are skipped by `os` and `cpu`.

## What you still need

| To build | You need |
|---|---|
| the JS bundle (`an init`, `an build`, `an plugins`) | Node 20.19+ and nothing else |
| iOS, tvOS, visionOS, watchOS, macOS | Xcode, plus **Rust** (`rustup`) |
| Android, Wear OS | the Android SDK with an NDK, a JDK, plus **Rust** |

The Rust core is compiled from the sources in this package by your own `cargo`
the first time you run a platform command; `rust-toolchain.toml` travels with it,
so rustup installs the toolchain and the targets without being asked. It is not
shipped prebuilt, and
[the installation guide](https://angular-native.github.io/guide/installing/)
says exactly why.

Documentation: <https://angular-native.github.io>.
MIT.
