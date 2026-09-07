# angular-native

React Native, but for Angular, and with the core in Rust.

Zoneless Angular with signals, running on an embedded JS engine; the UI tree,
the layout and the mounting onto **real native views** are Rust's job. No DOM,
no WebView, no zone.js. One core for iOS, Android, macOS, the TV, the headset
and both watches.

```text
  engine thread                                         UI thread
  ─────────────────────────────────────────────         ──────────────────────
  Angular (JS)                                          CADisplayLink
      │ Renderer2                                       Choreographer
      ▼                                                       │ asks for a frame
  __an_dom ──▶ binary buffer ──▶ ShadowTree                   ▼
                                     │ commit           MountSide
                               taffy (layout)                 │
                                     │ diff                   ▼
                               Frame { MountOp[] } ──────▶ HostRenderer
                                                        (UIKit / android.view)
```

An ordinary Angular component, with nothing special about it except that the
elements are native primitives:

```ts
@Component({
  selector: 'app-root',
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-view [style.gap]="'16'" [backgroundColor]="'#0b1020'">
      <an-text [fontSize]="28" [color]="'#f4f7ff'">angular-native</an-text>
      <an-switch [on]="alerts()" (onChange)="alerts.set($event)" />
      <an-view
        [backgroundColor]="'#1e2a4a'"
        [animate]="200"
        [translateX]="x()"
        (pan)="onPan($event)"></an-view>
      @if (seconds() >= 3) {
        <an-text [fontSize]="14">{{ seconds() }} seconds have gone by</an-text>
      }
    </an-view>
  `
})
export class AppComponent {
  readonly seconds = signal(0)
  readonly alerts = signal(true)
  readonly x = signal(0)
  onPan(event: NativePanEvent) { this.x.set(event.translationX) }
}
```

```ts
bootstrapNativeApplication(AppComponent)
```

That `<an-view>` ends up a `UIView` on iOS and an `AnViewGroup` on Android; the
`<an-switch>`, a `UISwitch` and a `MaterialSwitch`.

## Why not a WebView

A WebView renders a page, and a page can be made to *look* like the platform —
on the day it was written. What it will not do is age with the platform. A
lookalike switch does not change when the system's does, does not pick up the
new haptic, does not answer the accessibility API the way the real one does,
and does not know what the user set in Settings.

So `<an-switch>` **is** a `UISwitch` and a `MaterialSwitch`. Nothing is drawn by
hand to resemble a control, and where a platform has no equivalent the node is
not created at all: the layout leaves the gap it had measured and the host says
why, once, in the log. The long version is in
[guide/how-it-works](https://angular-native.github.io/guide/how-it-works/).

What follows from that is the interesting part: if the views belong to the
platform, the **layout** cannot. UIKit, AppKit and Android each lay out
differently and SwiftUI does not let you lay out at all, so layout leaves the
platform entirely and runs once, in the core, over taffy.

## In one minute

`an` is not confined to this repository. On any app out of `ng new`:

```bash
cargo install --path crates/an-cli   # puts `an` on the PATH, once per machine

cd my-app                            # an ordinary Angular project
an init                              # dependencies, tsconfig and entry point
an add ios                           # writes ios/Info.plist, yours from then on
an ios                               # to the simulator
```

It touches neither `src/main.ts` nor `angular.json`, so `ng build` and
`ng serve` keep working exactly as before. The whole flow — and why the
framework packages are vendored and the `.app` is not committed — is in
[guide/existing-angular-project](https://angular-native.github.io/guide/existing-angular-project/).

From inside this repository the same commands run against `examples/`:

```bash
cargo an dev                  # builds, launches in the simulator and reloads on save
cargo an dev --android        # the same, on the Android emulator
cargo an ios                  # once, without watching
cargo an android              # APK, emulator and launch
cargo an tvos                 # the TV: a tvOS .app and the Apple TV simulator
cargo an visionos             # the headset: a visionOS .app and the Vision Pro simulator
cargo an watchos              # the watch: a watchOS .app and its simulator
cargo an wearos               # the Android watch: a Wear OS APK and its emulator
cargo an macos                # the desktop: a macOS .app, on this very machine
cargo an build --release      # the bundle and nothing else
```

`an dev` also takes `--tvos`, `--visionos`, `--watchos`, `--wearos` and
`--macos`, all of them watching with hot refresh. `--macos` is the odd one out:
there is no simulator to launch into, so it closes the window that was open and
opens a new one.

Twelve commands, no nested subcommands, every option long-form. The whole
inventory, with the defaults and the four asymmetries between them, is in
[reference/cli](https://angular-native.github.io/reference/cli/).

It all goes through one binary. There is no `.xcodeproj` and no Gradle: `an`
drives `ngc`, esbuild, `cargo`, `swiftc`, `aapt2`, `javac`, `d8` and
`apksigner` itself, and the only file it leaves in your repository is a small
manifest.

The TV, the headset and both Apple watches need nightly:
`aarch64-apple-tvos-sim`, `aarch64-apple-visionos-sim` and
`aarch64-apple-watchos-sim` are tier 3 targets and their `std` is built on the
spot.

## Eight platforms, one bundle

| | Renders with | State |
|---|---|---|
| **iOS · iPadOS** | UIKit | all 25 primitives |
| **Android** | android.view + Material 3 | all 25 primitives |
| **macOS** | AppKit | all 25 primitives |
| **visionOS** | UIKit in a windowed scene | all 25 primitives |
| **tvOS** | UIKit + the focus engine | 20 of 25 — a TV has no switch and no slider |
| **Wear OS** | android.view | 18 of 25 |
| **watchOS** | SwiftUI | 17 of 25 |
| **HarmonyOS · Windows** | — | not started |

Each platform's page says which primitives it does not have and why, and every
one of those reasons is the SDK's rather than an opinion. The matrix in one
piece is [reference/components](https://angular-native.github.io/reference/components/).

Two of them are worth knowing before you write a template for them:

- **On the TV nothing is touched.** You navigate with the remote and the focus
  engine, and a control that cannot take focus cannot be pressed. That is not an
  implementation detail, it is the platform, and it changes what a template may
  assume. [platforms/tvos](https://angular-native.github.io/platforms/tvos/)
- **The Apple watch is the one host that is not a view hierarchy.** watchOS has
  no `UIView` to add subviews to, so the tree Rust maintains is mirrored into a
  model SwiftUI redraws. Everything else — the TV, the headset, the Android
  watch, the desktop — reuses the phone's host with different classes.
  [platforms/watchos](https://angular-native.github.io/platforms/watchos/)

## What is in it

**Twenty-five primitives**, from `an-view` and `an-text` through `an-switch`,
`an-select` and `an-date-picker` to `an-web-view`, `an-map-view` and
`an-video-view`. The framework draws none of them: on iOS they are `UISwitch`,
`UISlider`, `UISegmentedControl`, `UIDatePicker`, `UINavigationBar` and company,
with whatever look they have in that version of the system; on Android they are
Material 3's. The few that are assembled rather than mounted say so out loud and
are assembled out of system views all the same: `an-stepper` on Android, because
Material 3 defines no stepper, is two Material icon buttons and a Material
label; on macOS, `an-tab-bar` is an `NSSegmentedControl`, which is what Mac apps
actually use to change section.

**Icons** — SF Symbols on Apple platforms and Material Symbols on Android, by
name. Nothing is bundled on iOS; on Android it is, because what the platform
ships has been frozen since 2011 and is not Material 3's set.

**Composites** — `an-virtual-list` (a list that recycles its views),
`an-safe-area`, `an-native-stack` (stack navigation with a back gesture).

**Gestures** — `press`, `doublePress`, `longPress`, `pan`, `pinch`, `rotation`,
`swipeLeft`/`Right`/`Up`/`Down`. The recognisers are the system's, so the
thresholds for when a gesture counts are each platform's. On the desktop the
swipe is not a recogniser but an event travelling up the responder chain, so
only the views this host owns can catch it: put a `(swipeLeft)` straight onto a
system control and it warns when you subscribe, and says where to put it
instead.

**And where there is a mouse** — `(hover)` says when the pointer enters and
leaves a view, and `[cursor]` picks which of the system's pointers is shown over
it. Both are desktop-only: a finger has no shape, so iOS and Android do not read
them, and that is declared rather than forgotten.

**Transforms and animation** — `translateX`, `translateY`, `scale`, `rotate`,
and `[animate]="ms"` so that changes to that view stop being a jump. The
platform runs them on its own drawing thread, without coming back through
JavaScript on every frame.

**Other events** — `layout`, `safeArea`, `scroll`, `refresh`, `change`, `focus`,
`blur`, `submit`, `select`, `load`, `back`, `dismiss`, `hover`, and `crown` on
both watches.

**Angular** — AOT templates, signals, `@if`, `@for`, the router with parameters,
typed native modules, and hot refresh: saving swaps a component's code without
tearing the app down, so you stay on the same screen with whatever you had
typed.

**Plugins** — native modules written outside this repository. An npm package
with its Swift and its Java inside; the app declares it as a dependency and `an`
compiles and registers its part when it assembles the `.app` or the APK. If a
plugin does not cover the platform being built, the build stops and says so
rather than leaving a method that swallows the call.
[extending/plugins](https://angular-native.github.io/extending/plugins/)

## Verification without a device

```bash
./scripts/check-all.sh        # everything: 348 ok, and nothing else counts
```

That is the whole suite — the Rust tests, the duplicated lists, the example
apps, the accessibility trees, the plugins, an Angular project from outside, a
`.app` that is started and screenshotted, and the two cross-compilations.
Around thirty scripts run under it, and each one prints its own `ok` lines; a
script that cannot do its job says `skipped` rather than passing quietly.

```bash
cargo run -p an-bridge --example headless -- build/bundle/hello-angular/main.js 6
```

`headless` assembles the whole pipeline except the platform: it evaluates a
bundle, advances frames on a fake clock, simulates a press, a drag, a scroll and
a back, and prints the resolved tree. It is the quick way to debug without a
simulator, and it is what most of those scripts drive.

## Where things live

| Crate | What it does |
|---|---|
| `an-layout` | Style props to `taffy::Style`, the layout tree, measuring leaves |
| `an-core` | Shadow tree, mutations, commit, diff into `MountOp` |
| `an-host` | The `HostRenderer` and `TextMeasurer` traits, and the renderer's two halves |
| `an-bridge` | JS engine (QuickJS), binary protocol, native modules, engine thread |
| `an-ios` | UIKit host, measuring, controls, animation and the C surface. Also tvOS and visionOS |
| `an-android` | JNI host, measuring with `StaticLayout`, JNI entry points. Also Wear OS |
| `an-watch` | watchOS host: the tree mirrored into a model SwiftUI draws |
| `an-macos` | AppKit host: one `NSView` per node, system controls and the C surface |
| `an-cli` | The `an` tool: the thirteen commands, the plugin pipeline and the dev server |

| npm package | What it does |
|---|---|
| `packages/runtime` | JS prelude: console, timers, `AbortController`, the command buffer |
| `packages/platform-native` | `Renderer2`, the platform, `PlatformLocation`, navigation, modules |
| `packages/primitives` | Every primitive, control and composite |
| `packages/plugin-preferences` | `UserDefaults` and `SharedPreferences`, in a store of their own |
| `packages/plugin-clipboard` | The reference plugin: the clipboard, in Swift and in Java |
| `packages/plugin-biometrics` | Face ID, Touch ID and `BiometricPrompt` |
| `packages/plugin-keychain` | The keychain and the Android keystore |
| `packages/plugin-geolocation` | `CLLocationManager` and `LocationManager`, foreground and background |
| `packages/plugin-camera` | The camera and the photo library, presented by the system |
| `packages/plugin-notifications` | Local notifications, and the tap that launched the app |
| `packages/plugin-updater` | New JavaScript without a store release, with a bad bundle costing one launch |
| `packages/plugin-barcode` | `AVCaptureMetadataOutput`, full screen or as a live `<an-custom>` preview. Apple only, and it says why |

The Swift and Java shells are in `shells/` — `ios`, `tvos`, `visionos`,
`watchos`, `macos`, `android` and a `shared` one; Wear OS has none of its own,
because it reuses the phone's down to the `MainActivity`. `examples/` holds the
apps the checks drive.

## Decisions

- **Native views, not our own drawing.** Accessibility, IME, scrolling and the
  system look come free; the cost is one layer per platform for each primitive.
- **Node ids are assigned by JS.** Creating a node needs no round trip to the
  core, the same as Fabric's tags.
- **Layout in `taffy`**, not Yoga: it is Rust, and it brings flexbox, grid and
  block.
- **Zoneless, and not optionally.** Without zone.js there are no timers or XHR
  to patch inside the JS engine, which is 80% of NativeScript's pain.
- **One commit per frame.** Layout only runs in `commit()`, and the resulting
  `Frame` carries only what changed.
- **A binary buffer, not loose calls.** A `@for` over 200 rows is around 1,200
  mutations. One call each is 1,200 border crossings; a buffer is one. The
  protocol is twelve opcodes, little-endian, and a command the core turns down
  says which opcode failed and at what offset.
- **The JS engine lives on its own thread, and the UI thread waits with a
  deadline.** If the JS turn fits in what is left of the frame it mounts in that
  same frame; if it runs over, the UI thread carries on and mounts it when it
  comes. The separate thread is not for parallelism: the engine is given an 8 MB
  stack because Angular's router needs a little over 3 MB to get through one
  navigation, and iOS's main thread has 1 MB you cannot change.
- **JS has no clock of its own: it has one turn per frame.** Timers advance with
  the vsync, so the app's time is deterministic and a test can simulate ten
  seconds without waiting for them.
- **AOT always, never JIT.** `ngc` compiles the templates at build time and the
  Angular Linker resolves the packages published in partial mode; the device
  does not carry `@angular/compiler`.
- **Primitives as typed directives, not `CUSTOM_ELEMENTS_SCHEMA`.** The lax
  schema demands a hyphen in the name and, worse, turns off property checking:
  a typo'd `[bakcgroundColor]` would pass the compiler and fail silently on the
  device.
- **An `an-` prefix on every tag**, as Ionic does. Without it Angular refuses to
  self-close a tag named like an HTML element, and that cost two names: the
  dropdown ended up called `Picker` and the multi-line field `TextEditor`. With
  the prefix they are `<an-select>` and `<an-textarea>` again, and `<an-view />`
  self-closes like anything else.
- **From tag to core with a rule, not a table.** Strip `an-` and join in
  PascalCase: `an-text-input` is `TextInput`. Adding a primitive does not oblige
  anyone to write it into a translation list that can fall behind. The core's
  vocabulary does not follow the tag, though: `an-select` still travels as the
  `Picker` the Rust enum and the hosts know.
- **How big a control is, is the platform's decision.** A `UISwitch` is not the
  same size on iOS 17 as on iOS 26, nor with accessibility text sizes. They are
  asked at startup, on the main thread.
- **Events are registered only if the template asks for them.** `(press)` is an
  output over a cold observable: the recogniser is attached on subscribe.
- **`onLayout` is emitted by the core**, not by a platform: it is the one
  computing the frame, so it works the same on iOS and Android without being
  implemented twice.
- **A component's host is not a view.** If nobody gives it a style, a prop or a
  listener it never gets created and its children hang off the grandparent.
  Fabric calls it *view flattening*; here it is decided on the JS side, which
  sees the whole sequence before sending it.
- **Recycle, do not rebuild.** `an-virtual-list` mounts a fixed number of slots
  and creates or destroys none of them while scrolling: it changes what each one
  shows.
- **No `@angular/platform-browser`.** It drags in `DomAdapter`,
  `DomRendererFactory2` and the HTML sanitiser, all of them assuming a DOM
  exists. The platform this project provides instead is a couple of small files.

## What is not done

- **HarmonyOS has not been started.** Its SDK — DevEco — cannot be installed
  without accepting a licence by hand, so there is no way to compile or see
  anything here. The fit has been studied and it is good: Rust has an
  `aarch64-unknown-linux-ohos` target and ArkUI exposes a native C API that is
  imperative — create node, set attribute, add child — which is exactly this
  project's `MountOp` model. It would look like the Android host, not like the
  Apple watch's.

- **Windows has not been started.** The Rust targets are installed, but linking
  needs MSVC's libraries and running it needs a Windows machine: from a Mac you
  get at most an `.exe` linked with mingw, and a binary nobody has watched start
  is not a supported platform. It is waiting on a machine to try it on.

- **No plugins on the Apple watch.** That host has no plugin registry, so
  `an watchos` refuses to build an app that depends on one rather than shipping
  an app whose every call would be rejected at runtime. The `device` module is
  not a plugin — it is compiled into every host, and `Device.info()` resolves
  there too.

- **A plugin contributes methods, and exactly one kind of view.** A native
  module — a call that returns a promise — is the normal case. The one thing it
  can put in the tree is a view mounted through `<an-custom>`, which is not a
  primitive: it is never measured by its content, it does not exist on watchOS,
  and it has no props of its own beyond the box the layout gives it. The core's
  `NodeKind` stays a closed enum with frozen byte codes; the plugin's name
  travels as a prop.
  [extending/plugins](https://angular-native.github.io/extending/plugins/)

- **Hot refresh does not reach the framework.** Changing a component of the app
  keeps its state; changing `packages/` or a dependency forces a restart,
  because only one copy of Angular fits in the interpreter. It says so and
  restarts rather than showing you stale code.

Every platform keeps its own list of what is missing there, with the reason,
and those lists are the ones kept honest by the checks:
[tvOS](https://angular-native.github.io/platforms/tvos/) ·
[visionOS](https://angular-native.github.io/platforms/visionos/) ·
[watchOS](https://angular-native.github.io/platforms/watchos/) ·
[Wear OS](https://angular-native.github.io/platforms/wearos/) ·
[macOS](https://angular-native.github.io/platforms/macos/) ·
[iOS](https://angular-native.github.io/platforms/ios/) ·
[Android](https://angular-native.github.io/platforms/android/)

## Development

`NaiveMeasurer` approximates text measurement so the core can be tested with no
platform under it. On a device `UikitMeasurer` and `JniMeasurer` are what run,
and they cache by (text, font, available width): layout measures each node
several times per frame, and with no cache that is hundreds of border
crossings.

The tools find themselves: the Android SDK through `ANDROID_HOME` or its usual
location, and inside it the latest build-tools and platform. The NDK too —
where it is, which version, and what this host is called, which is
`darwin-x86_64` on a Mac and `linux-x86_64` on Linux — and from that the linker,
the archiver and the bindgen sysroot, handed to the `cargo` that cross-compiles.
None of it is committed: `.cargo/config.toml` used to hold all three written
out, which worked on exactly one laptop.

The documentation site is `docs-site/` — Astro and Starlight, English at the
root with an `es` locale — and it runs with `npm run docs`.
