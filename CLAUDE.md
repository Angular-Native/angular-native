# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Angular running on real native views, with the core in Rust. Zoneless Angular
executes inside an embedded QuickJS engine; the UI tree, the layout and the
mounting onto UIKit / android.view / AppKit / SwiftUI are Rust's job. No DOM, no
WebView, no zone.js. Eight platform targets share one core.

`README.md` is long and current — read it before making architectural changes;
its "Decisions" section explains *why* almost everything is the way it is, and
`docs-site/` holds the published documentation.

## Commands

Everything goes through one binary, `an` (crate `an-cli`). Inside this repo the
`cargo an` alias (`.cargo/config.toml`) runs it against `examples/`:

```bash
cargo an build examples/hello-angular   # ngc + esbuild only -> build/bundle/<name>/main.js
cargo an dev                            # build, launch in the iOS simulator, hot refresh on save
cargo an dev --android|--macos|--tvos|--visionos|--watchos|--wearos
cargo an ios | android | macos | tvos | visionos | watchos | wearos
cargo an plugins examples/secrets --platform ios   # plugin inventory + coverage check
```

The app argument is a directory under `examples/`; it defaults to
`examples/hello-angular` (`an macos` defaults to `examples/controls`). Add
`--release` for ngDevMode off + minified, `--no-launch` to build
without installing.

Outside the monorepo the same binary serves any `ng new` project:
`cargo install --path crates/an-cli`, then `an init`, `an add ios`, `an ios`.

### Tests and checks

```bash
./scripts/check-all.sh          # the whole device-free suite (~30 scripts, prints `ok` lines)
cargo test                      # the Rust unit tests alone
cargo test -p an-core           # one crate
cargo test -p an-watch snapshot # one test by name
./scripts/check-kinds.sh        # any single check script runs standalone
```

`check-all.sh` is the gate: it runs the Rust tests, the duplicated-list checks,
the example apps through headless, the accessibility trees, the plugins, an
external Angular project, a real macOS `.app` that is launched and
screenshotted, and the two cross-compilations. A script that cannot do its job
prints `skipped` rather than passing quietly. Scripts ending in `-device.sh`
need a plugged-in phone or watch and are *not* part of `check-all.sh`.

### Debugging without a simulator

```bash
cargo an build examples/hello-angular
cargo run -p an-bridge --example headless -- build/bundle/hello-angular/main.js 6
```

`headless` assembles the whole pipeline except the platform: it evaluates the
bundle, advances N frames on a fake clock, simulates press/drag/scroll/back and
prints the resolved tree with positions. Most check scripts drive it and grep
its output, so it is the fastest way to see what a template actually produces.

Docs site: `npm run docs` (Astro + Starlight, English at the root, `es` locale).
It is published to `Angular-Native/Angular-Native.github.io` by
`.github/workflows/docs.yml`, which pushes the built `docs-site/dist` there with
a deploy key; GitHub Pages serves that repository at the root of the org's
address, which is why the content needs no base path.

## Architecture

```
  engine thread                                  UI thread
  Angular (JS) -> Renderer2 -> __an_dom          CADisplayLink / Choreographer
      -> binary buffer -> ShadowTree (an-core)   -> MountSide -> HostRenderer
         -> taffy layout (an-layout) -> diff        (UIKit / android.view / AppKit)
         -> Frame { MountOp[] } ------------------>
```

| Crate | Role |
|---|---|
| `an-layout` | Style props to `taffy::Style`, layout tree, measuring leaves |
| `an-core` | Shadow tree, mutations, commit, diff into `MountOp`; props, colors, icons, a11y |
| `an-host` | The `HostRenderer` and `TextMeasurer` traits — the seam every host implements |
| `an-bridge` | QuickJS, the binary protocol, native modules, the engine thread |
| `an-ios` | UIKit host + C surface. Also tvOS and visionOS (see `src/family.rs`) |
| `an-android` | JNI host, `StaticLayout` measuring. Also Wear OS |
| `an-macos` | AppKit host, one `NSView` per node |
| `an-watch` | watchOS: no view hierarchy, so the tree is mirrored into a model SwiftUI redraws |
| `an-cli` | The `an` binary: commands, plugin pipeline, dev server, signing |

| npm package | Role |
|---|---|
| `packages/runtime` | `runtime.js`, the JS prelude: console, timers, `AbortController`, the command buffer |
| `packages/platform-native` | `Renderer2`, the platform, `PlatformLocation`, navigation, built-in modules |
| `packages/primitives` | Every primitive, control and composite |
| `packages/plugin-*` | Reference plugins (clipboard, biometrics, keychain), Swift + Java inside |

`shells/` holds the Swift and Java app shells (`ios`, `tvos`, `visionos`,
`watchos`, `macos`, `android`, `shared`); Wear OS reuses the phone's.
`examples/` holds the apps the checks drive. `build/` and `vendor/android/` are
generated and gitignored.

### The invariants the checks defend

Several lists are necessarily duplicated across languages, and drift in any of
them fails *silently* — an unrecognised prop just does nothing. Adding a
primitive or a prop means touching every link, and the check script that guards
it is the fastest way to find out which:

- **Primitive names** (`check-kinds.sh`): directive selector in
  `packages/primitives/src/primitives.ts` -> `NATIVE_KINDS` in
  `packages/platform-native/src/native-node.ts` -> `KIND` in
  `packages/runtime/runtime.js` -> `kind_from_byte` in
  `crates/an-bridge/src/protocol.rs`.
- **Style names** (`check-styles.sh`): the JS list vs the core's.
- **Props reaching both hosts** (`check-wrapper.sh`): a directive prop must be
  read by every host crate, not just one.
- **The site's address** (`check-docs-url.sh`): `DOCS_URL` at the root is the
  one place it is decided. Astro reads that file and derives the `CNAME` from
  it; the README's links, the `homepage` of every npm package and the URLs
  inside the CLI's error strings cannot share a variable, so they are written
  out and checked against it. Moving the site is editing `DOCS_URL` and running
  `./scripts/check-docs-url.sh --fix` — never editing a link by hand.

### Naming rules

- Every tag carries the `an-` prefix; the core's name is the tag with `an-`
  stripped and the rest joined in PascalCase (`an-text-input` -> `TextInput`).
  There is deliberately **no translation table**.
- Two natural-name exceptions the protocol froze: `an-select` travels as
  `Picker`, `an-textarea` as `TextEditor`.
- Primitives are typed directives, never `CUSTOM_ELEMENTS_SCHEMA` — the lax
  schema turns off property checking and a typo'd input would fail silently on
  the device.

### Things that will bite

- **The Android cross-compilation settings are discovered, not committed.**
  `Sdk::cargo_env` works out where the NDK is, which version and what this host
  is called, and hands them to the `cargo` it spawns; `.cargo/config.toml`
  holds only the `cargo an` alias, and `check-cargo-config.sh` keeps it that
  way. Set `ANDROID_NDK_HOME` to use one that is not under `<sdk>/ndk`.
- **The TV, the headset and both Apple watches need nightly with `rust-src`**:
  `aarch64-apple-tvos-sim`, `-visionos-sim` and `-watchos-sim` are tier 3 and
  their `std` is built on the spot. `rust-toolchain.toml` pins stable + the two
  iOS targets only.
- **Host crates compile empty off their platform** (`cfg(target_os = ...)`) so
  `cargo test` keeps working on a Mac. A change that compiles here may still
  break the real host — the cross-compilation step at the end of
  `check-all.sh` is what catches it.
- **`an` never touches `src/main.ts` or `angular.json`** in an external project,
  so `ng build`/`ng serve` keep working. Keep it that way. AOT always, never
  JIT: `ngc` compiles templates at build time and the device carries no
  `@angular/compiler`.
- **SDK root vs project root** (`crates/an-cli/src/workspace.rs`): inside the
  monorepo they coincide; outside, the SDK is this repo and the project is the
  user's. The rest of the CLI asks for `workspace.root` for SDK files and
  `workspace.build_dir()` for output and never learns which world it is in.
- **A plugin contributes methods, plus one kind of view.** `NodeKind::Custom`
  (code 26) is the single hole: the kind means "ask the host's plugin-view
  registry" and `an:view` says which name. Not a way to add a primitive — never
  measured by its content, absent on watchOS. `an watchos` still refuses to
  build an app depending on a plugin it cannot cover rather than shipping
  silent no-ops.
- **Hot refresh keeps state only for app components, and one file it cannot
  reach at all.** A component's definition is swapped in place and its instance
  survives. `packages/platform-native`, `packages/primitives` and the npm
  dependencies are the bundle's top half: only one copy of Angular fits in the
  interpreter, so the engine thread throws it away and evaluates the new bundle
  from cold *by itself* — nobody restarts anything by hand and the change is on
  screen on the next frame; what is lost is the component state.
  `packages/runtime/runtime.js` is the exception: it is `include_str!`-ed into
  `an-bridge`, so it lives in the native binary and no bundle can carry it.
  `an dev` recognises a save there (`dev::native_only`) and rebuilds and
  relaunches the app instead of reporting a reload that changed nothing. Rust
  and the Swift/Java shells are not watched at all.

## Conventions

Commit subjects are written as prose sentences, mostly lowercase, sometimes with
a `type(scope):` prefix (`fix(checks):`, `docs(site):`, `feat(examples):`) and
often without. They describe the behaviour change, not the files touched.

Comments in this codebase carry the *reasoning* — why a target is cfg'd out, why
a name was frozen, why a list is duplicated. Match that register when editing:
explain the constraint, not the mechanics.
