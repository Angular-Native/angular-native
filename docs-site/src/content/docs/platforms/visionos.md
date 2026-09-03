---
title: visionOS
description: Angular on the Vision Pro — the same core, the same host as iOS, and a window that is not a screen.
sidebar:
  order: 5
---

Angular on the Vision Pro: same core, same layout, same bundle and the same
host. visionOS ships UIKit with a `UIView` hierarchy and absolute frames, just
like iOS and tvOS, so `an-ios` compiles for the headset without rewriting
anything.

```bash
cargo an visionos                  # examples/hello-vision in the simulator
./scripts/check-visionos.sh
```

What actually differs is not the catalogue of controls — that is nearly all
there — it is **where the window comes from and what is behind it**.

## The third family

It is the third branch of the same `Family` enum in `crates/an-cli/src/ios.rs`,
and it inherits everything that module already knew: the `Info.plist` an
external project can contribute, the check that the plist agrees with the
project, and `build_dir()`.

## A window is not a screen

This is the part to look at head-on before concluding that "it looks the same
as iOS".

**There is no `UIScreen`.** It is marked `API_UNAVAILABLE(visionos)`, so
`UIScreen.main.bounds` does not even compile. An app does not fill a screen: it
fills a window the user hangs in the room, moves, and resizes by dragging a
corner whenever they feel like it. The only way to know anything about that
window is through its `UIWindowScene`, and the system only creates one if the
manifest declares who answers for it:

```text
  Info.plist                            shells/ios/Sources/SceneDelegate.swift
  UIApplicationSceneManifest  ──────▶   @objc(SceneDelegate)
    UISceneDelegateClassName              scene(_:willConnectTo:options:)
    = "SceneDelegate"                       └─▶ UIWindow(windowScene:)
```

That name is the Objective-C one, the one `@objc` puts on the Swift class. If
the two ever stopped matching, the scene would connect, nobody would create the
window, and **the app would start up black with no error at all**: that is why
`check-visionos.sh` compares the two strings.

**Absolute frames are not hurt by any of this.** The core never asked for a
screen: it receives a viewport and places things. What changes is that the
viewport is no longer constant for the life of the app. The shell asks for a
size on open — `requestGeometryUpdate(.Vision(size:))`, which is a *preference*:
the system may grant it or not — and the real size arrives through
`viewDidLayoutSubviews`, exactly as in the other two families. What does change
is what a template may write: **nothing in fixed points**. A width of `390` is
an assumption about a screen that does not exist here; `examples/hello-vision`
is written with percentages and `flexGrow`, and that is what the script checks.

**`deviceInfo.scale` comes back 0.** There is no screen scale to ask for: the
app is drawn for two eyes, at whatever distance the user put the window. Zero is
returned and said here, rather than inventing a `2.0` that somebody would end up
using to compute pixels.

## The background is glass, and covering it shows

A visionOS window already has a background: the glass the system draws, with its
blur and its shadow over the real room. The shell leaves the root transparent
(`view.backgroundColor = .clear`) instead of the black it sets in the other two
families.

A `[backgroundColor]` on the top container covers it entirely, and what is left
is an opaque slab floating in the living room. It is not an error — the app is
visible — and that is why it is checked: `check-visionos.sh` verifies that the
root container of `hello-vision` still paints nothing.

## Pointing with your eyes

visionOS **does have taps**: pinching with your hand counts as an indirect touch
and arrives through the same recognisers as on iOS, so no focus engine like the
TV's is needed. What changes is how the user knows what they are about to press:
they point with their eyes, and without a highlight there is no way to see where
they are pointing.

That highlight is drawn by the system, outside the app's process, but only if
the view asks for it with `hoverStyle`. A `UIView` with a tap recogniser does
not ask: the default is `nil`. So the host sets `automaticStyle` on every view
that becomes pressable (`crates/an-ios/src/hover.rs`). `automaticStyle` is the
system highlight with the shape UIKit infers from the view: nothing is drawn by
hand, and if visionOS changes how it looks in some release, this changes with
it.

And what is not there:

| Event | What happens on visionOS |
|---|---|
| `(back)` | There is no system gesture for going back: `UIScreenEdgePanGestureRecognizer` is not in the SDK and the window has no edges to drag. It is closed from its own bar, and inside the app the way back has to be a button in the template. This is said in the log. |

## What has been seen, and what has not

This matters and goes in its own section, because not everything above has the
same backing.

**Seen running in the visionOS 26.5 simulator:** the app starts, the window is
born from the scene, the tree mounts with the frames taffy computed, text is
measured with the real `UIFont`, the system glass shows through behind and
between the cards, and a tap reached the component's signal and changed the
label.

**Not verified:** I could not *trigger* that tap repeatably from outside.
`xcrun simctl` has no verb for the pointer, and a synthetic click on the
simulator window — with `cliclick`, with the window coordinates worked out
properly — does not reach the app: the visionOS simulator uses the mouse for
looking around and for pinching, and that input cannot be driven from the
command line the way the TV remote's can. What exists, then, is a screenshot of
the result of a tap and no way to repeat it in a script. While that holds, there
is no equivalent of `scripts/tv-remote.sh` here, and `check-visionos.sh` checks
the tap through the usual path: the headless renderer.

**Also not verified:** the gaze highlight. The host sets `hoverStyle` on
pressable views, but without being able to move the gaze from outside I could
not capture the highlight in place.

## What is missing

- **`deviceInfo.platform` still says `"ios"`** in all three families. An app
  that wants to adapt to the headset — or to the TV — has no way to know where
  it is. The fix is not in `an-ios`: the `NativeDeviceInfo.platform` type in
  `packages/primitives` declares the values it can take, and it has to be opened
  up there first.
- **Icon.** The `Info.plist` carries no `CFBundleIcons`: the visionOS icon is
  layered and lives in an asset catalogue compiled with `actool`. The app
  installs and launches; in the grid it appears with no icon.
- **Nothing volumetric.** This is a flat window in a 3D space, which is what
  visionOS calls a *window*. Neither a `volume` nor an immersive space: both are
  SwiftUI and RealityKit, and there is no `UIView` to mount in them, so they
  would not be this host but another one, as happened with the watch.
- **A real device.** All of this was seen in the simulator. A physical Vision
  Pro would require signing, and real gaze is not a mouse.
