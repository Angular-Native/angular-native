---
title: iOS · iPadOS
description: The reference platform — all 25 primitives on real UIKit views, and the handful of things UIKit made harder than it looks.
sidebar:
  order: 1
---

The platform everything else is measured against. All twenty-five primitives are
real UIKit views, every event is delivered, and both native modules and plugins
work.

```bash
an ios                      # examples/hello-angular in the simulator
an ios examples/controls    # the system controls
an dev                      # the same, watching, with hot refresh
```

Nothing else is needed: `aarch64-apple-ios-sim` is in every stable toolchain and
the SDK ships with Xcode. There is **no `.xcodeproj`** — an Xcode project would
only add a two-thousand-line file nobody can review in a diff.

## The shape of it

One crate, `an-ios`, serves three families: iOS, tvOS and visionOS. What
separates them is an enum and a handful of `cfg`s — see
[tvOS](/platforms/tvos/) and [visionOS](/platforms/visionos/) for what each one
loses or gains. On iOS nothing is missing: the "not available here" list is
empty.

The shell is five Swift files and a few hundred lines. `main.swift` calls
`UIApplicationMain` with no storyboard; the app delegate makes one `UIWindow`
from the screen's bounds with a single root view controller. There is no scene
delegate on iOS — that file is entirely visionOS.

The FFI is six functions: `an_runtime_new`, `_eval`, `_reload`,
`_set_viewport`, `_frame`, `_free`. Plugins add four more, and those are global
to the process rather than to the runtime.

### Two threads

The main thread owns every `UIView` — `an_runtime_new` returns null if it is not
called there. The JS engine, the shadow tree and taffy live on a worker with an
**8 MB stack**, and that number is measured rather than chosen: Angular's router
needs a little over 3 MB to complete one navigation, and at 2 MB the transition
advances seven events and stops. iOS's main thread has 1 MB you cannot change,
which is why the engine cannot live there.

Each frame the UI thread waits **at most 12 ms** of the 16.6 ms for the worker's
reply, then mounts whatever arrived and carries on. If the worker is still busy
no new tick is queued, or the queue would grow without bound. The clock is a
`CADisplayLink` in `.common` mode, and its timestamp is the app's only clock: JS
timers advance with frames, not on a thread of their own.

Panics get a hook that writes the site to stderr and flushes by hand, because a
panic inside an `extern "C"` function aborts and a buffered message would never
be seen.

### The mount model

A real UIKit hierarchy: one native view per node, `insertSubview(at:)` for
structure, and `setFrame` with the rect taffy produced. Auto Layout is never
used — it would be a second layout engine competing with the first.

Every created view gets `translatesAutoresizingMaskIntoConstraints = true`, and
that line has a story. It used to be `false`, which was harmless until the first
control with intrinsic constraints — a `UISearchBar`, a `UIDatePicker` — turned
Auto Layout on for the whole window and zeroed every frame. The symptom was the
entire screen piled into one corner.

A `UIScrollView` gets its `contentSize` from the core, which computes it. A
stack view clips to bounds, puts an entering screen on top regardless of tree
index, and holds a popping screen until its animation finishes.

## What each primitive becomes

The full mapping is in [Components](/reference/components/). Four choices are
worth the explanation:

- **`an-tab-bar` is a whole `UITabBarController`**, not a loose `UITabBar`.
  Since iOS 26 a loose bar draws itself through its own visual provider, and on
  iPad **it appears twice** — once at the top and once in the frame layout gave
  it. `.tabBar` mode is forced too, because `.automatic` can turn it into a
  sidebar on iPad.
- **`an-select` is a `UIButton` with `showsMenuAsPrimaryAction`**, not a
  `UIPickerView`. The picker view is the full-screen wheel, which is a different
  thing.
- **`an-map-view` is a real `MKMapView`.** The class is hand-declared, because
  the generated MapKit crate only covers macOS.
- **`an-alert` and `an-modal` mount a hidden placeholder** and present a real
  controller. A modal is a `UIViewController` and not a layered view because
  without one UIKit does not know something modal is in front: VoiceOver kept
  reading what was behind it, and presentation order against a
  `UIAlertController` depended on view creation order.

Icons are SF Symbols asked for by name. Nothing is bundled.

Non-uniform corner radii have no UIKit API, so the outline is drawn as a
`UIBezierPath` and installed as a shape-layer mask — and redrawn on every
resize, because a mask does not stretch.

## The safe area

Subscribing to `(safeArea)` registers the node and reports immediately; after
that the container's `safeAreaInsets` are re-read on every layout, and an event
goes out only when the four numbers actually changed.

`<an-safe-area>` applies them as **padding, not margin**, and the reason is
worth knowing: a view that moved itself out of the way with a margin would stop
being under the notch, would then report zero, would move back, and would do
that for ever.

## Two war stories the code carries

**Keyboard traits go through the runtime, not through generated setters.**
`[keyboardType]`, `[returnKeyType]`, `[autoCapitalize]` and `[autoCorrect]` are
applied by looking the implementation up with `class_getMethodImplementation`.
On iOS 26, `-[UITextField setKeyboardType:]` is **not in the class's method
table** — UIKit resolves it lazily — so `respondsToSelector:` says yes,
`class_getInstanceMethod` says no, the binding layer believes the second and
aborts. Every app with a text field crashed at launch.

**Video needs a retry loop.** Calling `play()` on a player that has not loaded
yet leaves the rate at zero for ever, with no error and a black layer, so the
call is re-issued every frame until it takes. The player view also has to be
resized in `flush`, because it is created after the frame was set.

## Gestures and events

Everything in [Components](/reference/components/) is delivered on iOS.
Mechanism depends on the node: target-action for controls, a delegate for
scrolling, gesture recognisers for views.

- `(press)` on a button is `TouchUpInside`. On a TV it is not — see
  [tvOS](/platforms/tvos/).
- A search bar's events are hooked onto its inner text field, not the bar.
- `(refresh)` is a real `UIRefreshControl`.
- `(back)` on an `an-stack-view` is a `UIScreenEdgePanGestureRecognizer` on the
  left edge, and it fires only on `Ended` — a cancelled drag must not navigate.
  The host only reports; undoing the navigation is the router's job.
- Taps force `userInteractionEnabled` on, because `UILabel` and `UIImageView`
  ship with it off.

**Nothing warns at subscribe time on iOS.** An event name the host does not
recognise is ignored in silence, deliberately: a template may carry a `(click)`
inherited from the web, and that is not a reason to make noise. The
subscribe-time warnings all belong to tvOS and visionOS.

## A modal's sheet, and where it rests

`[presentation]="'sheet'"` gives a `pageSheet` with a grabber. Where it may rest
is `[ios].detents`, and it is in the iOS object because a detent is UIKit's
idea: Android's `Dialog` is as tall as its content and has no list of stops to
be handed.

```html
<an-modal [visible]="open()" [presentation]="'sheet'" [ios]="{ detents: [180, 'large'] }" />
```

The list takes UIKit's three and no more: `'medium'` is `.medium()`, `'large'`
is `.large()`, and a number is `.custom(resolver:)` — a height in points,
measured from the bottom, iOS 16 and up. A height taller than the sheet can be
is brought down to that maximum rather than thrown away, because the resolver is
asked again on every rotation and "600 points" on a phone lying down means "all
the way up". The order is yours and is kept.

Saying nothing is still medium and large, which is what the system suggests. An
entry that is neither name nor a height above zero is dropped **with a warning
in the log**, once per value: the template's type already refuses those, so
getting there means an object assembled at run time. An empty list is not
passed on to UIKit, which answers one by throwing.

A list bound to a signal is re-applied to a sheet that is already up, so a
detent can be added or taken away without closing it.

Anything that is not a sheet is presented `overFullScreen` with a vertical
cover, chosen because the core has already laid the content out full screen, so
nothing has to be repositioned. That is also why a sheet resting at a small
detent shows the *top* of a full-screen layout: the content is not re-laid out
to the sheet's height.

On tvOS there are no detents at all — `UISheetPresentationController` is marked
`API_UNAVAILABLE(tvos)`, and asking for them says so once in the log.

## Text measurement

`boundingRectWithSize:` with line-fragment origin and font leading. CSS weights
100..900 map onto UIKit's -1..1 scale in nine steps. `boundingRect` knows
nothing about `lineHeight` or `numberOfLines`, so the line count is derived and
re-multiplied afterwards, and the result is rounded up so the last glyph is not
clipped.

The cache is not an optimisation: layout asks for min-content, max-content and
the final size, per node, per frame. It is keyed by text, size, weight, italic,
family, letter spacing and the available width rounded to an eighth of a point,
and cleared when the screen scale or the dynamic type setting changes.

Control sizes are measured once at startup on the main thread, by building each
control and asking `sizeThatFits`. Eleven are recorded; a slider, a progress
bar, a tab bar, a search bar and a segmented control stretch to the available
width.

## Build and run

```text
cargo build --target aarch64-apple-ios-sim -p an-ios
xcrun swiftc -target arm64-apple-ios17.0-simulator … -lan_ios
→ build/ios/<AppName>.app
```

The plist is checked against `angular-native.json` **before** anything compiles:
half a minute of cargo and swiftc is not worth burning to say the name does not
line up. Plugin coverage is checked first of all. The Swift sources — the
shell's, the shared ones, every plugin's and the generated registry — go into a
single `swiftc` invocation, sorted, so a plugin sees `AnPlugin` with nothing
imported and the command line does not change with filesystem order.

Entitlements, when a plugin asks for them, are embedded **inside the binary**
with `-sectcreate`, not put in a signature. On the simulator that is what works:
`keychain-access-groups` is a restricted entitlement and macOS refuses to run a
binary carrying it in a signature without a provisioning profile. The symptom
was the app refusing to start, with a denial that mentions entitlements nowhere.
Xcode does the same thing for the simulator.

Launch is `simctl boot` → open Simulator → `bootstatus -b` → terminate →
uninstall → install → launch. The wait matters: installing onto a half-booted
simulator hangs silently. The uninstall matters too: installing over an existing
app does not reliably replace the bundle, and the app starts with the old code.
The device is found by parsing `simctl list devices available -j` as JSON,
because `simctl` prints the udid before the name and grepping installs onto the
wrong simulator.

There is **no signing**, and **no physical device**. The Rust target is the
simulator's, the SDK is `iphonesimulator`, and the launch path is `simctl` end
to end. A real device needs the other route — an identity and a profile — and
that is not here.

## Resources in the bundle

An `<an-image [source]="'logo.png'">` — a path with no scheme — is a file that
travelled with the app, and it comes from `resources/` in the project:

```text
my-app/
  ios/Info.plist        an add ios writes it; yours from then on
  resources/logo.png    yours too; copied into the bundle under this name
  src/
```

It sits beside `ios/` and not under `src/assets/` or `public/` for a reason:
those two belong to the web build, `angular.json` decides what goes in them, and
`an` has promised never to touch `angular.json`. It would also put every favicon
and web font into a phone binary that has no browser to need them. Inside the
monorepo the same directory hangs off the example —
`examples/controls/resources/logo.png` — which is where the pink `A` at the top
of the controls screenshot comes from.

`an ios` copies the tree whole into the root of the `.app`, next to `main.js`,
keeping subdirectories: `resources/icons/logo.png` is asked for as
`icons/logo.png`. Dotfiles are skipped, so the `.DS_Store` the Finder leaves in
any directory it has opened never reaches a signed bundle. The copy happens
**before** the signature, because a signature covers every file in the bundle
and one added afterwards is one `installd` refuses.

Two things can go wrong, and neither of them is silent:

- A resource whose name is one the build writes itself — `main.js`,
  `Info.plist`, `dev-server.txt`, the executable — **stops the build**, naming
  the file. The copy would otherwise replace the app's own code, and nothing on
  the device would say why it launched into nothing.
- A name with nothing behind it is **warned once on the device**, with the name
  and where the file should have come from. It cannot be a build check: only the
  device knows which strings a template really produced, and half of them are
  computed. What it must not be is an empty `an-image`, which looks exactly like
  an image still loading, a colour that matches the background, or a frame of
  zero height.

The lookup is `UIImage(named:)` first — so an asset catalogue and the system's
own names go on working, and UIKit caches what it finds — and the file under the
bundle's resource path second, which is what makes a name with a slash in it
resolve at all.

## Onto a real iPhone, and into the store

```bash
an ios --physical                # signed, installed with devicectl
an ios --archive                 # .xcarchive and .ipa
```

Both are the same build with a different destination: `aarch64-apple-ios`
instead of the simulator target, the profile embedded in the bundle, and a real
signature over the whole thing. The entitlements move too — on the simulator
they live inside the binary, on a device they live in the signature — and they
are taken from what the provisioning profile grants, because the system gives an
app nothing its profile does not carry.

Neither of these has ever been run against a physical device or an Apple
Developer account by anybody working on this. What is checked is that they stop
before compiling when a credential is missing, and say which one. See
[Signing and distribution](/guide/signing-and-distribution/), which is explicit
about which paths have executed and which are written from Apple's
documentation.

## iPadOS

There is no separate iPadOS code path. It is iOS, with three places where the
iPad is acknowledged and all three exist because it broke without them:

1. **`UIDeviceFamily = [1, 2]` in the plist.** Without it the app declares
   itself iPhone-only, an iPad runs it in a scaled-up 320×480 compatibility
   window, and presenting any system controller makes iOS drop the scaling, so
   the content appears tiny in a corner.
2. **An action sheet must have a source on iPad.** The popover is anchored to
   the bottom centre of the container when the idiom is `.pad`, and deliberately
   not anchored on iPhone, where that would give the sheet a beak. Without the
   anchor UIKit does not warn — it crashes.
3. **The tab bar.** The controller, and forcing `.tabBar` mode, are both driven
   by iPad behaviour.

**Multitasking is not handled explicitly.** There is no scene manifest, no
`UIRequiresFullScreen`, no size-class or trait-collection handling. Resizing
works only generically: the new bounds go through `an_runtime_set_viewport` on
every layout, and `an-safe-area` re-reports on the next one. Supported
orientations are portrait and both landscapes.

## What is missing

- **No input accessory view.** The keyboard itself is handled — it is in the
  bottom inset, moving, and `<an-safe-area>` travels with it; see
  [The safe area and the keyboard](/guide/safe-area-and-keyboard/). What there
  is no way to ask for is the bar above it: a Done button, a next-field arrow,
  anything `inputAccessoryView` is for.
- **Six warn-once sites, all in accessibility**: an unknown role, an unknown
  state key, a role UIKit has no trait for, `checked: 'mixed'` — UIKit only
  knows checked and unchecked, so the value is left empty rather than rounded —
  and `expanded` and `busy`, neither of which has a trait. What is set and what
  is refused is in [Accessibility on Apple](/accessibility/apple/).
