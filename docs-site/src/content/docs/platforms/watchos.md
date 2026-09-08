---
title: watchOS
description: Angular on the Apple Watch — the one host that is not a view hierarchy, the digital crown, and the eight primitives the SDK does not have.
sidebar:
  order: 6
---

Same core, same layout, same bundle. What changes is who paints.

```bash
an watchos                          # examples/hello-watch in the simulator
an watchos examples/watch-controls  # everything the watch knows how to draw
an dev --watchos examples/watch-controls
./scripts/check-watchos.sh
```

## Why this is not the iOS host with a `cfg`

watchOS has no `UIView` hierarchy. The interface is SwiftUI and there is no way
around it: there is no container to add subviews to and no frame to move.

That collides head-on with this project's mounting model, which is imperative —
create a view, put it here, change its colour — while SwiftUI only accepts
state: you describe what there is and it decides what to redraw.

The way out is to mirror the tree Rust maintains into a model SwiftUI observes,
and turn every `MountOp` into a mutation of that model.

```text
  Rust                                   Swift
  ────────────────────────────────       ─────────────────────────────
  QuickJS ─▶ ShadowTree ─▶ taffy         Timer at 30 Hz
                 │ commit                       │ an_watch_runtime_frame
                 ▼                              ▼
           Frame { MountOp[] }            did the revision change?
                 │                              │ yes
                 ▼                              ▼
            WatchHost (model)   ──JSON──▶  AnTree (@Observable)
                                                 │
                                                 ▼
                                           ZStack + .position
```

`WatchHost` implements `HostRenderer` but owns no views: a `MountOp` lands in a
`HashMap<NodeId, WatchNode>` and every mutation bumps a revision counter. Once
per frame, `snapshot()` serialises **the whole tree** to JSON — a picture, never
a diff. A watch screen is ten or fifteen nodes, so `Codable` decodes it with no
hand-written parser and one FFI crossing replaces the hundreds that one call per
`MountOp` would cost. `AnTree` reads the revision first and returns early when
it has not changed, so a still frame decodes nothing and SwiftUI does not
recompose.

The JS thread runs with an 8 MB stack and a 25 ms frame budget: 33 ms at 30 Hz,
with 8 ms left over for SwiftUI to recompose. `CADisplayLink` does not exist on
watchOS, which is why the loop is a `Timer`.

**The layout is still taffy's.** There is not one `VStack`, one `HStack` or one
`.padding` in the whole shell. Two layout engines deciding the same thing means
whichever runs last wins, so every node is placed with `.frame(width:height:)`
plus `.position` — `.position`, not `.offset`, which would displace the view
from wherever SwiftUI had centred it — inside a `ZStack(alignment: .topLeading)`,
which is the coordinate system taffy hands over. `check-watchos.sh` enforces
this: a `VStack` or a `.padding` appearing in the files that draw the tree fails
the check.

Colours are resolved to 0..1 RGBA in Rust. Swift never parses `#rrggbb`.

## The toolchain, which is the part that may not be there

`aarch64-apple-watchos-sim` is a **tier 3** target: rustup lists it but ships no
prebuilt `std`. It has to be built on the spot, and that needs nightly:

```bash
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly
```

That is why `an watchos` calls `cargo +nightly` rather than the toolchain in
`rust-toolchain.toml`, which pins stable with the two iOS targets and nothing
else. `check-watchos.sh` skips the cross-compilation step — saying so — when it
cannot find a nightly with `rust-src`: somebody missing a toolchain should not
bring down the rest of the checks.

There is no `.xcodeproj`. `an watchos` calls `xcrun swiftc` directly with
`-sdk watchsimulator`, `-target arm64-apple-watchos11.0-simulator` and
`-parse-as-library` — without that last flag swiftc treats the SwiftUI `@main`
as a top-level script and it is silently never used. The sources are
`shells/watchos/Sources` plus one file from `shells/shared`.

**QuickJS compiles and runs on the watch untouched.** It was the big risk of the
port and it did not materialise.

## The seventeen it draws

Every one of them is the system control, not a drawing that resembles it: they
bring the haptics, the highlight and the crown behaviour watchOS gives them.

| Primitive | What it is on the watch |
|---|---|
| `an-view` | `ZStack(alignment: .topLeading)` — background, corners (one radius or four), border, opacity and whatever gestures the template asks for. |
| `an-text` | `Text`. Font, weight, italic, family, `letterSpacing`, underline and strikethrough, alignment and `numberOfLines`. Measured with the real `UIFont`. |
| `an-button` | `Button` with `.buttonStyle(.plain)`: the highlight and the haptics are the system's, the background is the app's. |
| `an-scroll-view` | `ScrollView(.vertical)`. The crown scrolls it because SwiftUI scrolls it — that is not something worth imitating. |
| `an-image` | From the bundle or from an `http(s)` URL, through an actor-backed cache; reports its natural size through `(load)`, which is what layout needs to place it. |
| `an-icon` | `Image(systemName:)`. The name is translated to an SF Symbol in Rust, by `an_core::icons`. |
| `an-switch` | `Toggle().labelsHidden()`. |
| `an-slider` | `Slider`. On the watch it comes with the minus and the plus at the sides, which is its shape there. |
| `an-stepper` | `Stepper`, the system's two buttons. |
| `an-progress-bar` | `ProgressView(value:)`. |
| `an-activity-indicator` | Indeterminate `ProgressView()`; with `[animating]` false it becomes `Color.clear`, the way `hidesWhenStopped` behaves. |
| `an-text-input` | `TextField` / `SecureField`. Tapping it opens the watch's **own** input screen — dictation, scribble or keyboard — and hands the text back. |
| `an-select` | `Picker().labelsHidden()`: the wheel the crown turns. There is no dropdown on a watch. |
| `an-date-picker` | `DatePicker`, the watch's dial picker. |
| `an-stack-view` | `ZStack` showing the last child, with transitions: `pop` comes in leading and leaves trailing, `none` is the identity, and the default is the reverse. `.easeOut` over 0.25 s. |
| `an-alert` | `.alert`, with its buttons and its `(select)`. With no buttons it gets an `OK`. |
| `an-modal` | `.sheet`, or `.fullScreenCover` when `[presentation]` is `fullScreen`. |

Two things a template has to know:

- **`an-text-input` needs `[style.height]`.** taffy measures it the way it
  measures a `<Text>` — one line — because on iOS a borderless `UITextField` is
  exactly that. On the watch the field always draws inside its own rounded
  container, **40 points** tall whatever frame it is given, so without an
  explicit height it eats the row below. The shell logs once when the frame
  comes in shorter than that. `.textFieldStyle(.plain)` does not remove the
  container: it was tried on watchOS 26 and changes nothing.
- **`an-alert` and `an-modal` leave the tree.** In SwiftUI they are not views
  you place, they are modifiers on the root, so Rust sends them separately, in
  `overlays`, and the shell hangs them off `RootView`. An `an-modal` **does**
  still take up room in the layout — the core decides that, exactly as on iOS —
  and on a 248-point-tall screen that is half the display: give it
  `[style.position]="'absolute'"` with the size of the screen, which is also the
  frame its contents get laid out in.

## The nine it does not

With the reason, which is almost always the SDK's and not an opinion. When the
tree asks for one of these the node is not created, the gap the layout measured
is left, and the host says so once — with the reason, not with a labelled box.
The shell draws `Color.clear` at the laid-out size on purpose.

| Primitive | Why not |
|---|---|
| `an-tab-bar` | A tab bar does not fit in 205 points of width. What a watch does is swipe between full-screen sections — a container, not a bar with a frame, so it is a different primitive. |
| `an-navigation-bar` | The strip at the top of a watch is already the system's: the time and the app's title. A bar of our own would paint under it or over it. |
| `an-segmented-control` | `SegmentedPickerStyle` is `@available(watchOS, unavailable)` in SwiftUI. What the watch uses instead is `an-select`. |
| `an-search-bar` | Search on a watch is a system screen, not a field with a magnifier. `.searchable` exists, but it is a navigation modifier, not a framed view. |
| `an-textarea` | `TextEditor` is `@available(watchOS, unavailable)`. Long text is dictated or scribbled, and `an-text-input` already gives you that. |
| `an-web-view` | WebKit is not in the watchOS SDK. |
| `an-map-view` | SwiftUI's `Map` does exist on watchOS, but it takes neither a centre nor a zoom from the app: it would show a place the template did not choose. |
| `an-video-view` | AVKit on watchOS ships neither `AVPlayerViewController` nor `VideoPlayer`. Its headers declare types and no playback view at all. |
| `an-custom` | The view a plugin brings is a native view somebody else built, and this host has no hierarchy to put one in: it mirrors the tree into a model SwiftUI redraws. The watch's plugin protocol has no `attach` for the same reason. |

`check-watchos.sh` checks that the supported list and the unsupported list
together cover the whole vocabulary, and that no primitive appears in both or in
neither.

## The digital crown

It is the watch's own control and the only one with no equivalent on a phone:
analogue, with inertia and with haptics, and used without a finger covering the
screen. A template asks for it like any other event:

```html
<an-view (crown)="turned($event)" (crownIdle)="stopped()"> … </an-view>
```

| Key | What it is |
|---|---|
| `delta` | How far it has turned since the previous event. |
| `offset` | Accumulated since the view took the crown. |
| `velocity` | Signed, in turns per second — SwiftUI's own `velocity`. |

`delta` is not something SwiftUI gives: `DigitalCrownEvent` carries `offset` and
`velocity`, and the shell subtracts. What a template almost always wants is
"move the value by however far it turned", and doing that subtraction in every
template would mean repeating it in every template. `(crownIdle)` arrives when
it stops, and is deliberately a different thing from a `(crown)` with a delta of
zero — SwiftUI's `onIdle` is a separate callback and there is nothing to invent.

The modifier is attached only to container nodes, and only when the node
actually listens for `crown`.

Three things worth knowing, and all three cost something to find out:

- **The crown goes to whoever has focus, and focus is singular.** That is not a
  shell decision, it is how watchOS works. A view with `(crown)` declares itself
  `focusable` and asks for the starting focus with `defaultFocus`, so if nobody
  else holds it, it keeps it. With several `(crown)` views, the first in paint
  order wins.
- **Assigning the `FocusState` by hand does not work.** It was tried from the
  root and from the node's own `onAppear`: SwiftUI swallows the assignment
  without a word if the view is not on screen yet, and the crown is left dead
  with nothing saying so. `defaultFocus` is what works.
- **A `ScrollView` keeps the crown.** If the `(crown)` view is inside one, the
  crown scrolls the list until the view is tapped. Every watch app behaves that
  way, but it is worth knowing: `examples/watch-controls` puts its crown screen
  **outside** an `an-scroll-view` on purpose. Touching a `Slider`, a `Stepper`
  or a `Picker` also moves focus to that control, and you have to tap the view
  again to get the crown back.

The controls that use it — `an-slider`, `an-stepper`, `an-select` — receive it
from the system without asking for anything: when they have focus, the crown
moves them. The crown accumulator is kept in a dictionary separate from the
control state, because one node can be both an `an-slider` and a `(crown)`
listener.

## Gestures

| Gesture | On the watch |
|---|---|
| `(press)` | `onTapGesture`. On an `an-button` the `Button` itself sends it, with its highlight and its haptics. |
| `(doublePress)` | `onTapGesture(count: 2)`. |
| `(longPress)` | A half-second `LongPressGesture`, in a `simultaneousGesture`. |
| `(pan)` | `DragGesture`, with `translation` and `velocity`. The phases are `begin`, `move` and `end`. |
| `(swipeLeft)`, `(swipeRight)`, `(swipeUp)`, `(swipeDown)` | The same `DragGesture`, reading the balance on release: 24 points or more on the dominant axis. |
| `(crown)`, `(crownIdle)` | The crown. |

Only what the template asks for is attached. One recogniser too many eats the
drag of the `ScrollView` underneath, and watchOS highlights whatever it thinks
is tappable, so wrapping everything in a gesture would make half the screen
flicker when it is brushed.

**`(longPress)` and dragging cannot be two separate gestures.** With
`.onLongPressGesture` on a view that also listened for `(swipeLeft)`, the long
press never arrived: the two fight over the finger and the drag won. It goes in
a `simultaneousGesture`, and the touch position comes out of the drag itself
rather than from a second zero-distance `DragGesture`, which was the same
problem again.

And what does not arrive, warned once per gesture:

| Gesture | Why not |
|---|---|
| `(pinch)`, `(rotation)` | `MagnifyGesture` and `RotateGesture` are `@available(watchOS, unavailable)`. Two fingers do not fit on 40 mm. |
| `(back)` | Outside a `NavigationStack` the watch gives no edge drag, and mounting one would put SwiftUI's layout inside taffy's. |
| `(refresh)` | You do not pull a list to reload on a watch: that is the crown, which already arrives as `(crown)`. |
| `(scroll)` | SwiftUI's `ScrollView` does not publish its offset on watchOS 11, which is this shell's minimum. |
| `(safeArea)` | The app owns the whole screen and the system reserves no queryable margin — so `an-safe-area` mounts and does nothing there. |
| `(focus)`, `(blur)` | Not yet: watch focus is the same focus that decides who owns the crown, and two owners would make it jump. |

## The state a finger moves

This is the part you cannot see and the easiest one to break. Both sides carry
the same value at different rates: a SwiftUI `Slider` needs a `Binding` it can
write to immediately — the finger is on it — while the real value lives in an
Angular signal on the other side of QuickJS and does not come back until the
next frame. Without something in between, the slider would jump backwards on
every drag, because each snapshot would put it back to the old value.

That something is `AnControls`, and it has one rule:

> if the value arriving from Rust **is different from the one that arrived last
> time**, the app changed it, and the app wins. If it is the same, whatever the
> finger did wins.

So a signal that changes the value from code shows up straight away, and a drag
is not overwritten by the echo of its own event.

Presentations need the same thing: pressing a button on an `an-alert` closes it
in SwiftUI while `[visible]` stays `true` until JS reacts to the `(select)`, and
the frame in between would reopen it. The dialog is recorded as closed until the
template catches up.

## Hot refresh

`an dev --watchos` works the way it does on the phone: the app connects to the
server over WebSocket and saving reloads the bundle without restarting. The
watch simulator shares the Mac's network, so the bundle is served at
`127.0.0.1:<port>`. The command has no `--device` of its own — a `--device` left
at the default phone is swapped for the default watch.

**The state survives**, and that was the fight worth winning, because the tree
is rebuilt whole thirty times a second. It survives because on a hot reload the
core keeps the tree mounted under the same ids, so `AnControls` never notices;
and when the reload is cold the host clears the tree, the ids disappear, and
reconciliation takes with it what no longer exists. Nothing has to be emptied by
hand, which is exactly where this would have broken.

## Driving the watch from outside

`xcrun simctl` **has no verb for this**: there is `io … screenshot`,
`io … recordVideo`, `ui`, `spawn`, `push`… and none of them sends a tap or a
turn. It is the same situation as the Apple TV remote, and the way out is the
same as `tv-remote.sh`: move Simulator's own mouse.

```bash
./scripts/watch-input.sh tap 104 200          # a tap, in watch points
./scripts/watch-input.sh hold 104 120 900     # a long press
./scripts/watch-input.sh drag 104 220 104 60  # a drag: scrolls a list
./scripts/watch-input.sh turn -12             # twelve crown steps
./scripts/watch-input.sh shot /tmp/a.png
```

The thing that took longest to find, and is written at the top of the script:
**the mouse wheel only turns the crown while the pointer is over the window's
"Crown" button**, not over the screen. Over the screen absolutely nothing
happens — not even a warning — and it is easy to conclude the crown cannot be
driven from outside at all.

It has the same two conditions as the TV remote: the Mac's screen cannot be
locked, and Simulator stays in the foreground while the script runs.

## Nothing fails in silence

Four warnings, each once per case and never once per frame — at 30 Hz, one
warning per frame is an unreadable log:

- **A prop nobody reads.** The per-kind allow-list is in `reads()`, next to the
  code that uses it; `[cursor]`, which only the desktop host has a pointer for,
  is one of the props that lands here today. The `ios:` and `android:` prefixes
  and `ng-version` are exempt.
- **A prop that cannot be painted**, with the reason, in `unpaintable()`. It is
  deliberately not the same message as the one above: "nobody reads this yet" is
  a gap somebody can close and "there is nothing here to draw on" is not, and an
  app author who cannot tell them apart waits for a release that is never
  coming. A `[borderWidth]` on an `an-alert` is the case that exists today — the
  system presents the dialog and the app hands it a title, a message and
  buttons, not a frame.
- **An event that cannot be delivered**, warned once per kind and event name.
- **A primitive that is not drawn**, with the SDK's reason.

Event payloads are flat only. A nested object or an array is rejected with a
log, because the value type on the wire cannot represent them.

## Native modules

`device` is registered, so `Device.info()` resolves and `'watchos'` — a value of
`NativePlatform` that until then no host produced — is a value an app can
actually read.

The four fields it answers come **from the shell**, handed to
`an_watch_runtime_new` as JSON alongside the control sizes: `systemVersion`,
`model` and `scale` are `WKInterfaceDevice`'s, which is WatchKit, has no Rust
binding, and would be four trips through Objective-C for what Swift settles in
one line. Android does exactly this with `AnHost.deviceInfo()`.

`platform` is the one field the shell does **not** send. That is the crate's
word — this host cannot be running anywhere but a watch — for the same reason
`an-ios` takes it from the `cfg`: asking somebody else would only be room to get
it wrong. With nothing handed over, the call is rejected saying so, rather than
answering an object with holes in it that would read as real data.

## Resources in the bundle

An `<an-image [source]="'logo.png'">` — a path with no scheme — is a file that
travelled with the app, and it comes out of `resources/` in the project, the
same directory as on the [phone](/platforms/ios/#resources-in-the-bundle).
`an watchos` copies the tree into the root of the `.app`, beside `main.js`,
keeping subdirectories and skipping dotfiles.

That is where iOS puts them too, and not for the same reason. On the phone the
root is where `UIImage(named:)` searches; on watchOS that call resolves names
against an asset catalogue, and a bundle assembled by bare `swiftc` with no
`.xcodeproj` has none. What decides it here is the shape of the bundle: a watch
`.app` is flat — `WKApplication`, no `Contents/` — so `Bundle.main.resourcePath`
is the `.app` itself, which is already the directory the shell reads `main.js`
out of and the one it builds an image's path from. A `Resources/` subdirectory
of our own would be a directory nothing in Foundation looks into.

Four names are reserved, as on iOS and for the iOS reason — the bundle is flat,
so even the executable shares a directory with the resources: `main.js`,
`Info.plist`, `dev-server.txt` and `AngularNativeWatch`. A resource that would
land on one of them stops the build, naming the file. There is no signature for
the copy to come before: the watch simulator takes the bundle unsigned.

A `[source]` naming a file that is not there is **warned once**, with the name
and where the file should have come from — the same sentence `an-ios` says,
because it is the same mistake. It cannot be a build check: only the device
knows which strings a template really produced. And it must not be an empty
view, which on a watch is indistinguishable from an image still loading.
`examples/watch-controls` carries one now, next to the heading of its first
screen; until the build could take it there, that example had no image at all,
since an example in this repository cannot be made to depend on a server being
up.

## What is missing

- **A view a plugin brings.** A plugin's *methods* work here: `an-watch` has
  the same registry every other host has, and `an watchos` now refuses a build
  only when a plugin says it cannot cover a watch — see
  [Plugins on the Mac and the watch](/extending/plugins-on-the-mac-and-the-watch/).
  What has nowhere to go is `<an-custom>`. This host mirrors the tree into a
  model SwiftUI redraws, so there is no view hierarchy to mount a plugin's view
  into, and the watch's `AnPlugin` protocol has no `attach` for the same reason:
  a stand-in that handed over nothing would only hide that until run time. A
  module name reached at run time that is not there is rejected with the name
  *and* the reason there is nothing under it, which is not the message a typo
  would get.
- **Animation and transforms.** `[animate]`, `translateX`, `scale`, `rotate`.
  In SwiftUI these are `withAnimation` and `.offset`/`.scaleEffect`, but the
  model is rebuilt whole on every snapshot and an animation needs to know where
  it came from. The stack view already animates its screens in and out, which is
  the most visible case.
- **Per-side borders.** There are none, here or anywhere else in the project.
  `[borderWidth]` is one number and it is drawn; `borderTopWidth` and its three
  siblings are *layout* styles — taffy resolves them, they inset the children
  and they never reach a host as something to paint. SwiftUI has no shape for
  them either, so drawing them on the watch alone would mean butting four
  rectangles together and having the watch show an edge the phone does not.
- **The whole snapshot instead of the mutations.** Every changed frame
  serialises the whole tree. For ten or fifteen nodes it does not show. When a
  long list turns up, the ops will have to be sent on their own, and what
  changes is `snapshot.rs` and the Swift `Decodable`, not the host.
- **Complications and notifications.** A different system surface, with its own
  lifecycle. They share nothing with this.
- **A real device.** There is no device path anywhere: no `aarch64-apple-watchos`
  target, no codesigning, no `devicectl`. Everything here was seen in the
  simulator, on an Apple Watch Series 11 (46 mm), the default device.

## Accessibility, and the one thing that cannot be proved

Roles, traits and values are translated in Rust and merely attached in Swift, on
every node rather than per `case`. What is set and what is refused is in
[Accessibility on Apple](/accessibility/apple/).

The gap worth stating here is a testing one: the other three Apple hosts can be
read from outside by walking the accessibility tree. watchOS cannot — there is
no route into a watch simulator's tree — so nothing proves a screen reader would
announce any of it. The Rust test only checks the snapshot carries the right
names.

## Walls that were expected and were not there

Documented because they were on the list of things that might stop this:

- **The simulator takes the bundle unsigned.** `simctl install` asks for no
  signature, just like iOS. A real device would.
- **`@main` on a SwiftUI `App` works with bare `swiftc`**, no `.xcodeproj`,
  as long as `-parse-as-library` is passed.
- **The `Info.plist` needs `WKApplication`.** It is what distinguishes a
  watchOS 7-and-later app from the old "WatchKit App + WatchKit Extension"
  pair. Without that key `simctl` installs the bundle and then cannot find
  anything to launch. The plist also carries `WKWatchOnly` — standalone, with no
  companion iPhone app — `UIDeviceFamily [4]` and portrait only.
- **Text measurement is the real thing.** watchOS ships a trimmed UIKit with no
  `UIView` but **with** `UIFont` and Foundation's string drawing, so the watch
  measurer measures the same as the iOS one and with the same weight table:
  CSS 100..900 mapped onto UIKit's -0.8..0.62. Off-watch it falls back to the
  naive measurer so `cargo test` runs on a Mac. Control sizes cannot be asked
  for — there is no `UISwitch` to query — so Swift hard-codes them and passes
  them in at startup.
