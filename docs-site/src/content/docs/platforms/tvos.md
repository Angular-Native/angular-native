---
title: tvOS
description: Angular on the Apple TV — the same host as iOS, and a platform where nothing is touched.
sidebar:
  order: 4
---

Angular on the Apple TV: same core, same layout, same bundle and — this is what
separates it from the watch — the **same host**. tvOS ships UIKit with a
`UIView` hierarchy and absolute frames, which is exactly this project's mounting
model, so `an-ios` compiles for the TV without rewriting anything.

```bash
cargo an tvos                      # examples/hello-tv in the simulator
cargo an tvos examples/controls    # the system controls, to see the gaps
cargo an dev --tvos                # the same, watching, with hot refresh
./scripts/check-tvos.sh
```

Hot refresh works as it does on the phone: the app connects to the server over
WebSocket, and saving a change in a component changes the label on the TV
without restarting and with the state — the seconds counter — where it was.

What does change, and it is not cosmetic, is **how it is driven**: there are no
taps.

## Splitting by family

`ios.rs` assembles the `.app` for all three UIKit families — iOS, tvOS and
visionOS. What separates the TV from the phone fits in an enum, `Family`, and it
is not much:

| | iOS | tvOS |
|---|---|---|
| Rust target | `aarch64-apple-ios-sim` (tier 2) | `aarch64-apple-tvos-sim` (**tier 3**) |
| swiftc triple | `arm64-apple-ios17.0-simulator` | `arm64-apple-tvos17.0-simulator` |
| `xcrun` SDK | `iphonesimulator` | `appletvsimulator` |
| `Info.plist` | `shells/ios/Resources` | `shells/tvos/Resources` |
| `.app` suffix | — | `TV` / `.tv` |
| Rust `std` | comes prebuilt | built with `-Z build-std` |

There is no `tvos.rs`. It would be a second copy of the same `swiftc`
invocation, and the copy is what falls behind the day somebody fixes something
in only one of them. Splitting here is also what makes tvOS inherit, for free,
what that module already knew about projects outside the monorepo:
`workspace.build_dir()`, the `Info.plist` a project can contribute, and the
check that the plist agrees with the project.

**The Swift sources are the same** (`shells/ios/Sources`). The only thing not
shared is the `Info.plist`, because the keys each family wants are nothing alike:
tvOS's carries `UIDeviceFamily = 3` and does not carry `LSRequiresIPhoneOS`,
`UILaunchScreen` or orientations, which are the phone's.

**The name and the identifier carry a suffix.** `AngularNativeTV` and
`dev.angularnative.playground.tv`. Without it, `an tvos` would overwrite the
`.app` `an ios` had just left, and installing one would uninstall the other.
`an add tvos` writes the project's `Info.plist` with the same suffix, taken from
the same place (`Family::suffix`), so the two cannot drift apart.

### The toolchain, which is the part that may not be there

`aarch64-apple-tvos-sim` is a **tier 3** target: rustup lists it but ships no
prebuilt `std`. It has to be built on the spot, and that needs nightly, as
watchOS already did:

```bash
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly
```

`rquickjs-sys` ships no pregenerated bindings for tvOS either; they are
generated with bindgen, and tvOS only had to be added to the list in
`an-bridge/Cargo.toml`. **QuickJS compiles and runs on the TV untouched.**

Having the SDK is not enough to *run*: the simulator runtime is a separate
download (`xcodebuild -downloadPlatform tvOS`). When it is missing, `an tvos`
assembles the `.app` and says so in those words, instead of saying it cannot
find the device, which would send you looking in the wrong place.

## Focus is the platform

**On a TV nothing is touched.** The remote moves an invisible cursor between the
views that declare themselves focusable, and the centre button presses whichever
is focused at the time. UIKit decides that path geometrically, from the views'
frames — and our frames are absolute and computed by taffy, so the focus engine
gets exactly the grid the template describes and there is nothing to translate.

What you do have to do is **declare yourself**. `-[UIView canBecomeFocused]`
returns `NO` by default, and a view that returns `NO` cannot be pressed on a TV:
the recogniser attaches, nothing fails, and the button simply never answers.
`canBecomeFocused` can only be changed by subclassing, so on tvOS the host
creates `an-view` as a subclass of its own, `AnFocusableView`
(`crates/an-ios/src/focus.rs`):

```text
  an-view  ──▶  AnFocusableView : UIView
                 canBecomeFocused = do I have any recogniser on me?
                 didUpdateFocusInContext: ──▶ focus / blur towards the core
```

The answer is computed on the spot rather than kept in a counter: the view is
focusable if it has any gesture on it, and UIKit already keeps that list. So
when the core removes the last listener, the view stops being focusable by
itself.

`an-text` and `an-image` are `UILabel` and `UIImageView` and **still cannot take
focus**. On a TV they have to be wrapped in an `an-view`; `events::attach` says
so in the log as soon as somebody puts a `(press)` on something the remote
cannot reach.

### The centre button is not a touch

`UIGestureRecognizer.allowedPressTypes` decides which remote button a recogniser
answers to, and the SDK documents its default as "platform dependent". It is set
by hand: `(press)` and `(longPress)` ask for `UIPressType.Select` — the centre
button — and `(back)` asks for `UIPressType.Menu`, which is the remote's "back"
and what on the phone is the drag from the left edge. The direction buttons are
never claimed: they belong to the focus engine, and taking them would leave the
remote unable to move around the screen.

**And an `an-button` does not answer `TouchUpInside`.** This was the bug that
cost seeing it with the button on screen: `(press)` on an `an-button` was wired
to `UIControlEvents::TouchUpInside`, which is what UIKit sends when a finger
lifts off the screen. On a TV there are no fingers: the remote presses the
centre button over whatever is focused and UIKit sends
`PrimaryActionTriggered`. Nothing failed — the target-action pair was installed,
the button took focus, went white and lifted the way tvOS does — and pressing it
did absolutely nothing. tvOS uses `PrimaryActionTriggered`; iOS keeps
`TouchUpInside`.

### Each control paints its own highlight, not the system

tvOS **has no `UIFocusEffect`** — it is marked `API_UNAVAILABLE(tvos)`, it
belongs to iOS — and the system paints nothing of its own over a plain view. On
tvOS the highlight is each control's business: a `UIButton` lifts, goes white
and casts a shadow because UIKit draws it.

A focused `an-view`, by contrast, **does not look any different**. No border and
no scale are drawn here by hand: that would be exactly the imitation this
project does not do. That is why `examples/hello-tv` carries two `an-button`s
and one `an-view`: moving focus between the two buttons is the only thing a
screenshot can show, and the `an-view`'s own counter is what proves focus
reached it too.

## What does not exist on tvOS

This list is written by hand in `crates/an-ios/src/family.rs`, and for a reason:
`objc2-ui-kit` generates the bindings for every Apple platform without looking
at the SDK's availability annotation, so `UISwitch::new(mtm)` **compiles** for
tvOS and what fails is the class lookup at runtime, already inside the simulator
and with the process aborting. The list is the SDK's annotation brought into
Rust, and it is what separates "cannot be done" from "closes without saying
why".

| Primitive | What happens on tvOS |
|---|---|
| `an-switch` | `UISwitch` is not in the SDK. On a TV a switch is a focusable row you press; there is no equivalent system control. |
| `an-slider` | `UISlider` is not there. The nearest thing that is, `UIProgressView`, only shows a value: it cannot be dragged. |
| `an-stepper` | `UIStepper` is not there. |
| `an-date-picker` | `UIDatePicker` is not there: the system asks for dates with a screen of its own, not with a control that fits in a frame. |
| `an-web-view` | The whole of WebKit is outside the tvOS SDK. The `web` module leaves the binary through `cfg`, and not only because of the class: its `#[link(name = "WebKit")]` would make the link step look for a framework that is not there. |

When the tree asks for one of these, the node **is not created**, a gap the size
the layout said is left, and it is said once per kind:

```
angular-native: Switch is not available on tvOS: UISwitch does not exist on tvOS. …
```

And what is not there on the events side:

| Event | What happens on tvOS |
|---|---|
| `(pinch)`, `(rotation)` | The remote's surface is single-touch, and `UIPinchGestureRecognizer` and `UIRotationGestureRecognizer` are not in the SDK. It warns and attaches nothing. |
| `(refresh)` | `UIRefreshControl` is not there, and on a TV there is nothing to pull. |
| `(back)` | It does arrive, but through the remote's **menu button**, not through an edge drag: `UIScreenEdgePanGestureRecognizer` is `API_UNAVAILABLE(tvos)`. |
| `[sheet]` on `an-modal` | There is no sheet: `UISheetPresentationController` is not there and the presentation covers the whole screen, which is how it is presented on a TV. It says so. |

`(pan)` and the four `(swipe)`s **do still work**: the remote's surface sends
indirect touches and UIKit recognises them like a finger's.

And what is there, working as it does on the phone: `an-text`, `an-image`,
`an-scroll-view`, `an-text-input`, `an-textarea`, `an-button`, `an-tab-bar`,
`an-segmented-control`, `an-search-bar`, `an-select`, `an-navigation-bar`,
`an-icon` (SF Symbols), `an-activity-indicator`, `an-progress-bar`, `an-alert`,
`an-modal`, `an-map-view`, `an-video-view`.

## The measurements are a TV's

The viewport is **1920x1080 points**, not an iPhone's 393. A `fontSize` of 28
here cannot be read from the sofa; `examples/hello-tv` uses 76 for the title and
30 for the body.

The edges of a television are cropped — *overscan* — and Apple asks for **90
points at the sides and 60 top and bottom**. The framework does not add that:
the template does, with `paddingHorizontal` and `paddingVertical`, like any
other margin. `an-safe-area` does not help here: tvOS's `safeAreaInsets` are
zero.

## Driving the remote from outside

`xcrun simctl` **has no verb for the remote**. There is `io … screenshot`,
`io … recordVideo`, `ui`, `spawn`, `push`… and none of them sends a press; the
binary has nothing like it hidden either. What does exist is the Simulator's own
keyboard input: the tvOS simulator translates the arrow keys into focus moves
and return into the centre button.

`scripts/tv-remote.sh` is that and nothing more: it activates Simulator, checks
that it really came to the front, and sends the key codes with `osascript`. On a
tvOS simulator there is nothing to enable first: `I/O ▸ Input ▸ Send Keyboard
Input to Device` appears disabled, because the keyboard goes to the remote
anyway.

```bash
./scripts/tv-remote.sh down down select    # two down and press
./scripts/tv-remote.sh menu                # the remote's "back"
```

It has three conditions that cannot be dodged and that the script checks before
sending anything, because otherwise the press goes somewhere else and nobody
finds out:

- **The Mac's screen cannot be locked.** With the session locked no application
  can be brought to the front and the keys reach nowhere.
  `CGSSessionScreenIsLocked` says so, and the script stops there.
- **Simulator has to stay in the foreground.** It is a real limitation of this
  route: while the script runs, the keyboard is its.
- **The Apple TV window has to be the focused one inside Simulator.** With an
  iPhone open at the same time, the keys go to whichever window was in front —
  another simulator — and absolutely nothing happens on the Apple TV. The script
  raises the window by its title and checks it kept focus; `AN_TV_WINDOW`
  changes what it searches by.

## What is missing

- **Icon.** The tvOS `Info.plist` deliberately carries no `CFBundleIcons`: a
  tvOS app's icon is a layered icon plus the top shelf image, and both live in
  an asset catalogue compiled with `actool`. There is none here yet, so the key
  stays out rather than pointing at a name that does not exist. The app installs
  and launches; in the grid it appears with no icon.
- **`an-switch` and `an-slider` have no substitute.** Today they leave a gap and
  a warning. What belongs on a TV is a focusable row you press and a row that
  answers left/right, but that is a new primitive and a vocabulary decision, not
  a `cfg`.
- **Plugins are compiled with their iOS sources**, which is all they declare. If
  one uses API tvOS does not have, linking stops with swiftc's error; `an tvos`
  warns before starting so it does not arrive as a surprise. No plugin declares
  a separate tvOS part yet.
- **A real device.** Everything here was seen in the tvOS 26.5 simulator. A
  physical Apple TV would require signing, and the real remote has a touch
  surface the simulator does not reproduce.
