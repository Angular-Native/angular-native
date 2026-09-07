---
title: macOS
description: Angular as a desktop app — real NSViews, a window that resizes, a pointer with a shape, and the one platform you can check without starting anything.
sidebar:
  order: 3
---

Angular running as a desktop app: same core, same layout, same bundle, and real
`NSView`s underneath.

```bash
an macos                     # assembles the .app and opens it on this machine
an macos examples/desktop    # the pointer and the swipe
an macos examples/media      # the map and the video
./scripts/check-macos.sh
```

Nothing else is needed: `aarch64-apple-darwin` is in every stable toolchain and
the SDK ships with Xcode. There is no simulator to boot, no device to find, no
`.xcodeproj`. With no app argument, `an macos` builds `examples/controls`. The
bundle lands at `build/macos/AngularNativeMac.app`, and `--release` and
`--no-launch` both work.

`an add macos` gives a project its own `macos/Info.plist`, and from then on the
name and the identifier are the project's: `MyApp` becomes `MyAppMac`, with
`.mac` on the end of the identifier, the same decoration `an add tvos` and
`an add visionos` apply. The `.app` goes to the project's build directory and
not to the SDK's, like everything else `an` produces.

The app is **ad-hoc signed** — `codesign --force --sign -` — and that step is
not optional. Unsigned, macOS kills the process on the first `mmap` of generated
code, which is exactly what QuickJS does, with a `Killed: 9` and no explanation.

## Why this *is* the iOS host with different classes

The watch needed a host of its own because watchOS has no `UIView` hierarchy.
That does not happen here. AppKit is imperative, `NSView` exists, and this
project's mounting model — create a view, put it here, change its frame — fits
as it is. So `an-macos` is `an-ios`'s brother: one native view per mountable
node, the frame written directly because taffy already resolved the layout, and
`MountOp` applied to the hierarchy.

It is a **separate crate** all the same, sharing no code with `an-ios`. Two
tables are duplicated knowingly: the SF Symbols name translation, and the
hand-declared bindings for `WKWebView`, `MKMapView` and `AVPlayerView` — six
methods each, rather than compiling two entire generated framework crates into
every build. `objc2` checks every signature against the real one when the
message is sent, so what is saved in compile time is not paid for in safety.

Threading is the same split as iOS: the main thread owns the views, QuickJS and
the shadow tree live on a worker with an 8 MB stack, and the frame budget for
waiting on the engine is 12 ms — deliberately not lowered for 120 Hz displays.

The FFI is six functions — `an_runtime_new`, `_eval`, `_reload`,
`_set_viewport`, `_frame`, `_free`. The header says what it is: the phone's,
minus the plugins.

## What macOS draws

All twenty-six mountable primitives. Twenty-two are a system control, two are
assembled from system views, one — the navigation header — macOS puts where it
keeps it, outside the view tree, and one is not this crate's to build at all:
`an-custom` is the hole a **plugin** mounts its own `NSView` through. The
inventory lives in
`crates/an-macos/src/support.rs`, as a table rather than scattered through the
`create` match, so it can be read in one go; `scripts/check-macos.py` compares it
against the core's enum so it cannot fall behind.

| Primitive | macOS |
|---|---|
| `an-view`, `an-stack-view` | `NSView` (`AnFlippedView`) |
| `an-text` | `NSTextField`, configured as a label |
| `an-text-input`, `an-search-bar` | `NSTextField`, `NSSearchField` |
| `an-textarea` | `NSTextView` |
| `an-image`, `an-icon` | `NSImageView` — icons are SF Symbols, by name |
| `an-scroll-view` | `NSScrollView` |
| `an-button` | `NSButton` |
| `an-switch`, `an-slider` | `NSSwitch`, `NSSlider` |
| `an-activity-indicator`, `an-progress-bar` | `NSProgressIndicator` |
| `an-segmented-control`, `an-stepper` | `NSSegmentedControl`, `NSStepper` |
| `an-select`, `an-date-picker` | `NSPopUpButton`, `NSDatePicker` |
| `an-alert` | `NSAlert` |
| `an-web-view` | `WKWebView` |
| `an-map-view` | `MKMapView` |
| `an-video-view` | `AVPlayerView` |

Two are assembled, and which is which is said out loud:

- **`an-modal`** is an `NSView` above the root. What is genuinely modal on macOS
  is a sheet (`beginSheet:`) or an `NSPanel`, and both take the content out of
  the window where the layout put it. Since the core already lays the modal out
  full screen, a layer gives the same result without two coordinate systems
  arguing. The system dialog — `an-alert` — *is* a real `NSAlert`.
- **`an-tab-bar`** is an `NSSegmentedControl`. macOS has no tab bar; a segmented
  control is what Mac apps actually use to change section. `NSTabView` will not
  do: that is the document tab, with its own frame and background.

The map and the video came out cheaper here than on the phone, for the same
reason: in AppKit they are views. `MKMapView` inherits from `NSView` and so does
`AVPlayerView`, so they join the tree like anything else. UIKit has no video
*view* — it has an `AVPlayerViewController` — and putting that into the tree
means making it a child of the presiding controller and repositioning its view
by hand every frame, because it is born after the frame is set.

**The native map needs no key.** MapKit JS is what asks for one, and that is a
different product. What does need permission is `[showsUser]`, declared in the
`.app`'s `Info.plist` as `NSLocationUsageDescription`: without that key the
system denies location on its own, the dot never appears, and there is no error
to look at. `check-macos.py` does not let one go without the other.

The `create` match has **no wildcard**: adding a `NodeKind` to the core breaks
this build rather than silently mounting nothing.

## The header is the title bar

`an-navigation-bar` is not drawn inside the content: on a Mac the header of the
screen you are on lives at the top, in the window's title bar, and painting
another one below would be two. What is not thrown away is what the template
wrote. The `[title]` ends up as the window's title, which is where a Mac user
looks for it:

```html
<an-navigation-bar [title]="'Notes'" />
```

and the window becomes "Notes". When that screen unmounts, the window gets back
the title it had. It is applied from `flush` rather than from the property
setter, because at that point the view may not be in a window yet.

The node measures **zero by zero**, and that is written down rather than falling
out of nobody measuring it: a header that is not drawn but reserves forty-four
points leaves an empty strip under the real title bar, and whoever sees it will
have no idea where it came from.

What the title bar does not have is a back button. `[showsBack]` and
`[backTitle]` are dropped for that reason, and `(back)` warns when you subscribe:
on a Mac you go back with the menu or with a button of the app's own, not with
an arrow in the header.

That is why the inventory has two more categories beside "system control",
"assembled from system views" and "macOS does not have it". `Elsewhere` — it is
honoured, but not with a view — is what the header uses, and it is the only one
that does. `Plugin` is `an-custom`: a real `NSView`, but one whose class this
crate has never heard of, so it is neither a `Native` (the name would be a lie)
nor an `Assembled` (nothing is assembled — the view arrives finished). No
primitive is `Missing`. If anyone ever declares one, `check-macos.py` says so and asks for
the warning path to be written again: a node that mounts as an empty box in
silence is exactly what this repository does not allow.

## The three desktop things

### The window resizes, and does it live

On a phone the viewport changes on rotation, which happens once in a while. Here
it changes while somebody drags a corner, sixty times a second.

`viewDidLayout` calls `an_runtime_set_viewport` on every step of the drag, and
the function discards repeated sizes: each real change is a round trip to the
engine thread that also waits for whatever was in flight, and that cannot be
paid on every `layout()`. The shell calls without thinking about it and the
filter is in Rust, which is where the last one is known.

The frame clock changes too: the `CADisplayLink` is asked of the **view**, not
the screen. A Mac has several screens and they can refresh at different rates,
so the right one is the screen the window is on, and the view is what knows
that. It runs in `.common` mode so it keeps beating while the window is dragged
or a menu is open, which in AppKit run on a separate event loop. Asking a view
for a display link is also what pins the deployment target at macOS 14.

The window is 720×820, titled, closable, miniaturisable and **resizable**, with
its frame autosaved so the size survives a relaunch. No minimum size is ever
set. There is exactly one window, and the app quits when it closes: nothing
exposes a second one.

### There is a mouse, not fingers

The gestures are AppKit's recognisers, which are not UIKit's. `press`,
`doublePress`, `longPress`, `pan`, `pinch` and `rotation` each have one and
behave like the iOS ones, with the system's thresholds. Pinch and rotation
report a `velocity` of zero, because the trackpad does not supply one.

**Swiping does exist, though it is not a recogniser.** AppKit has no
`NSSwipeGestureRecognizer`. The gesture is there; what is missing is something
to hang on a view. A swipe arrives as a loose event, `swipeWithEvent:`, which
travels up the responder chain like a keypress. The system produces it from
whatever the user configured in Trackpad, with its threshold and its finger
count, so the judgement of when it counts stays the platform's.

Catching it requires the method to be on the class, which is why it lives with
`AnFlippedView` — the view this host mounts for `an-view`, `an-stack-view`,
`an-modal` and a scroll view's document — and not with the other gestures. An
`NSButton` is the system's and you cannot add a method to it with the app
running, so a `(swipeLeft)` put directly on a control warns when you subscribe
and says where to put it instead. That is not a hole: a `swipeWithEvent:` a
control does not handle goes up to the next responder, which is its parent view,
so a swipe over a button reaches the `<an-view>` wrapping it. And one of our own
views that is not listening for that direction **also passes it on** rather than
swallowing it.

Which direction each sign is comes from `NSEvent.h` and not from a guess:
`deltaX` −1 is rightwards and 1 leftwards; `deltaY` −1 is downwards and 1
upwards. It lives outside the part that touches AppKit so it can be tested with
no trackpad, and a test pins it.

**Hover and cursor are here, and they are the system's.** Both live on
`NativeVisual`, so on all twenty-five primitives:

```html
<an-view [cursor]="'pointer'" (hover)="over.set($event.hovered)">
```

- `(hover)` delivers `{ hovered, x, y }`. One output with a boolean rather than
  two, because what is underneath is one thing too: an `NSTrackingArea` gives
  entry and exit down the same path.
- `[cursor]` takes seven CSS names — `default`, `pointer`, `text`, `crosshair`,
  `grab`, `grabbing`, `not-allowed` — and each is a system `NSCursor`. None is
  drawn. An unrecognised name warns once and leaves the pointer alone. The
  resize cursors are not there: the ones macOS has always had are deprecated,
  and their replacements arrived in macOS 15, later than this host's minimum.

Both are mounted on an `NSTrackingArea` rather than on the view, and that is the
trick: **the owner of a tracking area does not have to be the view**. With a
separate owner object, `(hover)` and `[cursor]` work over a system `NSButton`
exactly as they do over one of ours, subclassing nothing. Setting the cursor the
other way — `addCursorRect:cursor:` — would have required overriding
`resetCursorRects`, which is precisely what you cannot do to a system control.

The area carries `InVisibleRect`, which is what makes it unnecessary to rebuild
on every layout: AppKit keeps it stuck to the view's rectangle. Without that, an
area would stay the size the view had when you subscribed, and resizing the
window — which on a desktop happens constantly — would have the pointer entering
and leaving where there is nothing.

And it carries `ActiveInActiveApp`, not `ActiveAlways`: on a Mac, controls only
light up on hover when the app is in front, and this host is not going to be the
exception that behaves differently from the rest of the desktop.

None of this takes away what the system controls do by themselves — an
`NSButton` highlights on hover and an `NSTextField` turns the pointer into an
I-beam — because they are real controls and not drawings.

Neither `(hover)` nor `[cursor]` reaches iOS or Android, and that is not a gap:
a finger has no shape. `check-wrapper.sh` declares them as such and demands that
the desktop host *does* read them. See
[Props and the native wrapper](/guide/native-wrapper/).

### Scrolling has no delegate

`(scroll)` arrives with the same `{ x, y }` iOS sends, counting downwards, so a
component listening for it needs to know nothing about which desktop it is on.
Getting there is not the same, though. UIKit has `UIScrollViewDelegate`; AppKit
has nothing of the sort. An `NSScrollView` reports movement by posting
`NSViewBoundsDidChange` from its **clip view**, and only if that clip view has
been asked to — `postsBoundsChangedNotifications` is off by default, and its
absence is a subscription that never fires with no error anywhere.

The offset is the clip view's `bounds.origin`, and it counts downwards for the
same reason everything else here does: the document view is flipped.

`scripts/check-macos.sh` moves the clip view by 90 points on a running window
and asserts the template says 90. A synthetic scroll-wheel event would not do:
a wheel event is a *request*, and how far the system carries it, over how many
frames and with what elasticity is AppKit's business, so a check built on one
measures the system's scrolling rather than whether the notification arrives.

### The menu is the system's

On a phone there is no menu. On a Mac there always is, it lives in the bar at
the top and not inside the window, so it does not fit in the tree the core
mounts: there is no `NodeKind` for it. The shell puts it there and it is not
exposed to Angular.

That it **exists** is not cosmetic. macOS's editing shortcuts — ⌘X, ⌘C, ⌘V, ⌘Z,
⌘A — are not implemented by `NSTextField`: the menu dispatches them down the
responder chain. Without an Edit menu, copy and paste in an `an-text-input` does
not work, with no error and nothing to look at. That is why the minimum menu
includes Edit and not just Quit.

## The four points a label keeps to itself

Layout measures text with `boundingRectWithSize:`, which measures **the text**
and nothing else. An `NSTextField` draws that text inside its cell, and the cell
keeps a few points on each side. Today it is four.

Four points sound like nothing and are exactly the worst size of error. The
label has `wraps` set, so what does not fit goes to the next line, and that line
is outside the height layout reserved for one. The text is not clipped: it comes
out **empty**, with no error, no trace, nothing to look at. It shows up the
moment a label lands in a box its exact size — that is, in any
`align-items: center`, which is where it appeared.

The measurement is asked of the system at startup, on the main thread, alongside
the natural size of every other control, instead of being written down: it is an
AppKit measurement and it changes with the version and with the accessibility
settings, exactly like the height of an `NSSwitch`. Text itself is measured with
`NSAttributedString`'s `boundingRectWithSize:` — not `NSString`'s, which on
AppKit lacks the line-fragment options the iOS host uses — with CSS weights
100..900 mapped onto `NSFont`'s -1..1 scale, cached by text, font and available
width. Italic is not resolved in the measurer: `NSFont` has no italic factory,
it goes through a descriptor trait, and that is more work than it is worth so
far.

## Accessibility

The six accessibility props land in the `NSAccessibility` protocol, and the
whole story — including the sharpest edge on this platform, that overriding
*anything* costs the view the role AppKit was computing for it, so the primitive
writes the implied role back by hand — is in
[Accessibility on Apple](/accessibility/apple/).

Two props have no AppKit shape and are refused rather than approximated: the
`summary` role, which is a VoiceOver-on-iOS idea with no `NSAccessibility`
counterpart, and the `busy` state, whose nearest relative is the
`AXBusyIndicator` *role* — a different view, not a state of this one.

Reading the tree from outside needs the Accessibility permission granted by
hand; the example is `examples/a11y`.

## Native modules

`device` is registered, so `Device.info()` resolves here. Three of the five
fields mean exactly what they mean on a phone; two do not, and rather than
returning the nearest-looking number the difference is written down:

| Field | On macOS |
|---|---|
| `platform` | `'macos'` |
| `systemVersion` | `NSProcessInfo.operatingSystemVersion`, as `major.minor.patch`. Not `operatingSystemVersionString`, which reads "Version 15.3.1 (Build 24D70)" and is prose. |
| `model` | The hardware identifier from `sysctl hw.model` — `Mac15,7`, `MacBookPro18,3`. AppKit has no `UIDevice.model`, which on a phone answers a device *class*; here that word would be "Mac" for every Mac ever made. |
| `scale` | `backingScaleFactor` of the screen the app **started on**. A window can be dragged to a display with another factor and this will not follow: it is read once, on the main thread, because `NSScreen` cannot be touched from the engine's thread. With no screen at all it is `0.0`, the same answer visionOS gives, rather than an invented `2.0`. |
| `locale` | `NSLocale.currentLocale`, BCP 47. |

There is no second module: everything else would be a plugin, and see above.

## How it is checked

It is the one platform that can genuinely be checked without starting anything
external: the app runs on the same machine that compiled it.
`scripts/check-macos.sh` starts it, lets it mount the tree and **asks it for a
screenshot**, over `examples/controls`, `examples/desktop` and `examples/media`.

The same property is what makes `scripts/check-dev-macos.sh` possible, and it is
the only place the development loop is checked end to end: the server serves, the
`.app` is built with the URL inside it, the app connects, a file is saved and the
window is then asked what it is showing. What it has to be showing is the new
template *and* the state from before the save — `hello-angular` puts a running
timer and an `@if` that unfolded at three seconds on screen, and a restart sends
both back to nothing.

Screenshot mode is macOS-only and driven by environment variables read at
startup: `AN_SCREENSHOT=<path>` plus `AN_SCREENSHOT_FRAMES` (40 by default),
`AN_SCREENSHOT_PRESS=x,y`, `AN_SCREENSHOT_SWIPE=x,y,dx,dy`,
`AN_SCREENSHOT_HOVER=x,y`, `AN_SCREENSHOT_WINDOW=1`,
`AN_SCREENSHOT_RELOADS=n` and `AN_DUMP_TEXT=1`, which logs the strings the
mounted `NSView`s are really showing — the one thing a PNG cannot be asked. The three synthetic inputs fire inside the wait in a
fixed order — press at a quarter of the frames, swipe at a third, hover at a
half — and the shot is taken at the end. It uses `cacheDisplay(in:to:)`, which
needs no screen-recording permission, and `CGWarpMouseCursorPosition`, which
needs no accessibility permission. If nothing ever mounts it gives up after 600
frames rather than hanging.

One consequence of `ActiveInActiveApp`: the hover assertion is **skipped**, and
says so, when the app cannot be brought to the front. A skip is reported and is
not a pass.

## Shipping it to another Mac

```bash
an macos . --sign                # Developer ID, hardened runtime
an macos . --notarize --dmg      # submitted, stapled, and packed
```

The ad-hoc signature above is what lets the app run **here**. Anywhere else,
Gatekeeper wants a Developer ID signature and a notarisation ticket, and the
hardened runtime that notarisation requires is not a flag change: it forbids
mapping writable executable memory, which is the first thing the JS engine does.
So the signed build declares `com.apple.security.cs.allow-jit`. Without it the
app is killed on startup with a `Killed: 9` that mentions no entitlement — the
same failure the ad-hoc signature exists to prevent, wearing the one face nobody
recognises.

The `.dmg` is signed and notarised in its own right, because the image is the
file that gets downloaded. Packing one is checked here; signing and notarising
need a paid Developer ID certificate and have never been run. See
[Signing and distribution](/guide/signing-and-distribution/).

## What is missing

- **A plugin view is not measured by its content.** `<an-custom>` mounts what a
  plugin registered with `AnPluginViews.register`, filling the box the layout
  gave the node — and a node given no size comes out at zero, which looks like
  a plugin that does not work. A name nobody registered mounts nothing and says
  so once, naming the name. See
  [a view a plugin brings](/extending/plugins/#a-view-a-plugin-brings).
- **A plugin's entitlements need a real signature.** The host loads plugins, but
  an ad-hoc-signed `.app` carrying a profile-backed entitlement is killed at
  launch by the system, so an unsigned build drops those keys and warns naming
  the key, the plugin and the flag that restores them. See
  [Plugins on the Mac and the watch](/extending/plugins-on-the-mac-and-the-watch/).
- **Unequal corner radii collapse.** A `CALayer` has one radius, so all four
  rounded corners take the largest of them, warned once.
- **`enabled` on anything that is not an `NSControl`** does nothing: AppKit has
  no `userInteractionEnabled`.
- **`animateDelay` is folded into the duration** — `NSAnimationContext` has no
  delay — and `animateEasing` is applied only for `ease-in-out`.
- **`color` on `an-switch`, `an-activity-indicator` and `an-progress-bar`** is
  the system accent colour and is not settable per view.
- **`resizeMode: 'cover'` fits inside instead of cropping**, because
  `NSImageView` cannot crop.
- **`sheet` on `an-alert`** only changes the alert style: macOS has no action
  sheet. The alert is presented with `beginSheetModalForWindow:` and never
  `runModal`, which would freeze the event loop that drives the frame.
- **`(dismiss)` on `an-modal`, `(refresh)` on `an-scroll-view` and `(back)` on
  either the stack view or the navigation bar** all warn at subscribe time, each
  with its reason.
- **`safeArea` answers once with all-zero insets**, deliberately.
- Props accepted and dropped with a one-time warning include `keyboardType`,
  `returnKeyType`, `autoCapitalize`, `autoCorrect`, `secureTextEntry`,
  `thumbColor`, `maximumTrackColor`, `refreshing`, `bounces`, `transition`,
  `showsBack`, `backTitle`, `presentation` and `unselectedColor`. Anything
  outside that list prints "unknown prop", which the checks treat as a failure.
