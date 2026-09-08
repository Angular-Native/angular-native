---
title: Events
description: Every output a template can bind, the exact payload each one carries, when it is delivered, and which platforms cannot deliver it — with the reason they give when you subscribe anyway.
sidebar:
  order: 7
---

An output here is not a DOM event with a cast on top. Each one is typed, each
one carries a declared payload, and each one is a **cold** observable: the
platform's recogniser or listener is attached when Angular subscribes and let go
when the view is destroyed. A view nobody is listening to costs nothing.

```ts
import type { NativePanEvent } from '@angular-native/primitives'

onPan(event: NativePanEvent) { … }
```

## When they arrive

Never in the middle of a frame. A native event is queued and handed to
JavaScript at the **start of the next frame**, so everything that happened
between two vsyncs is processed in one turn and produces one commit.

A drag gives one `(pan)` per frame, not one per touch sample. The extra samples
would each cost a change-detection pass and produce a tree identical to the one
that frame is going to mount anyway.

## On every primitive

These live on the base class, so all 25 have them.

### Gestures

| Output | Payload |
|---|---|
| `(press)` | `{ x, y }` — points, relative to the view that took it |
| `(doublePress)` | `{ x, y }` |
| `(longPress)` | `{ x, y }` — once only, when the system decides it counts |
| `(pan)` | `{ x, y, translationX, translationY, velocityX, velocityY, state }` |
| `(pinch)` | `{ scale, velocity, state }` — `scale` is relative to the gesture's start |
| `(rotation)` | `{ rotation, velocity, state }` — radians since the gesture began |
| `(swipeLeft)` `(swipeRight)` `(swipeUp)` `(swipeDown)` | `{ x, y }` |

`state` is `'begin' | 'move' | 'end' | 'cancel'`. `cancel` is not a failure — the
system took the gesture away because another one won. `velocity` is in points
per second, useful for coasting on after the finger lifts.

`translation` is measured from where the finger started, not from the previous
event.

:::note[`(rotation)`, not `(rotate)`]
`[rotate]` is already the transform and a class cannot have two members with one
name. On the wire the event is still called `rotate`; only the output's name
differs.
:::

The full story — recognisers per platform, what `cancel` means in practice, and
how these compose with `[animate]` — is in
[gestures and animation](/guide/gestures-and-animation/).

### Layout and the screen

| Output | Payload | |
|---|---|---|
| `(layout)` | `{ x, y, width, height }` | The resolved frame, relative to the parent, every time it changes. |
| `(safeArea)` | `{ top, right, bottom, left }` | The margins the system reserves — **including the keyboard**. |

`(layout)` is emitted by the **core**, not by any platform: the core is what
computes the frame, so it works identically everywhere and was never implemented
twice.

`(safeArea)` changes on rotation, on going into split screen, and while the
keyboard animates. See [the safe area and the keyboard](/guide/safe-area-and-keyboard/).

### Focus and the pointer

| Output | Payload | |
|---|---|---|
| `(focus)` | `{ value? }` | `value` only when the view is a text field. |
| `(blur)` | `{ value? }` | |
| `(hover)` | `{ hovered, x, y }` | Desktop only; refused with a reason everywhere else. One output rather than two, because one `NSTrackingArea` delivers both. |

Focus used to live on `an-text-input` alone, because on a phone focus belongs to
the keyboard. On a TV it is the whole platform — the remote walks the focusable
views and there is no other way to highlight the one under the cursor — so it
moved to the base.

### The watch

| Output | Payload | |
|---|---|---|
| `(crown)` | `{ delta, offset, velocity }` | The digital crown, while it turns. |
| `(crownIdle)` | — | It stopped. Without this there is no way to know when to stop. |

`delta` is the change since the last notification, which is nearly always what
is wanted; SwiftUI only hands over the running total, and `offset` is that
total since the view took focus.

It is on the base class and not on a control because the crown goes to
**whichever view holds focus**, whatever it is — the equivalent of rolling a
mouse wheel over something.

## Per primitive

| Primitive | Output | Payload |
|---|---|---|
| `an-switch` | `(onChange)` | `boolean` |
| `an-slider` | `(valueChange)` | `number` |
| `an-stepper` | `(change)` | `{ value: number }` |
| `an-segmented-control` | `(change)` | `{ index: number }` |
| `an-select` | `(change)` | `{ index: number }` |
| `an-date-picker` | `(change)` | `{ value: number }` |
| `an-tab-bar` | `(select)` | `number` — the index |
| `an-text-input` | `(valueChange)` | `string` |
| | `(submit)` | `string` — the return key |
| `an-textarea` | `(change)` | `{ value: string }` |
| `an-search-bar` | `(input)` | `{ value: string }` |
| | `(submit)` | `{ value: string }` |
| `an-scroll-view` | `(scroll)` | `{ x, y }` |
| | `(refresh)` | — pull to refresh |
| `an-image` | `(load)` | `{ width, height }` — the image's intrinsic size |
| `an-stack-view` | `(back)` | — the edge gesture, or Android's button |
| `an-navigation-bar` | `(back)` | — the back item |
| `an-modal` | `(dismiss)` | — dismissed by the user rather than by the prop |
| `an-alert` | `(select)` | `number` — which button |

Six of these hand over the value itself rather than an object —
`(onChange)`, `(valueChange)` on both the slider and the text input,
`(select)` on the tab bar and the alert — because there was nothing else in the
payload to keep it company and `$event.value` on every one of them was noise.
The rest keep the object, so that adding a second field later is not a breaking
change.

## What a platform cannot deliver

An event a platform cannot give **warns when you subscribe**, and the warning
carries the reason. It does not stay silent, and the reason is nearly always the
SDK's rather than a decision made here.

### tvOS

| Event | Why not |
|---|---|
| `(pinch)`, `(rotation)` | The remote's surface is single-touch, and `UIPinchGestureRecognizer` and `UIRotationGestureRecognizer` are not in the tvOS SDK. |
| `(refresh)` | `UIRefreshControl` is not in the SDK, and a television has nothing to pull. |

`(pan)` and the four swipes do work — the surface reports a drag. `(press)`
arrives from the centre button, so a view that cannot take focus can never be
pressed.

### iOS, iPadOS and visionOS

| Event | Why not |
|---|---|
| `(hover)` | The pointer is the desktop's. See below. |
| `(crown)`, `(crownIdle)` | The digital crown is the Apple Watch's; there is no wheel to turn here. |
| `(back)` on `an-stack-view` | visionOS only: `UIScreenEdgePanGestureRecognizer` is not in that SDK and the window has no edge to drag in from. |

Anything else that reaches the host with nothing to attach it to is answered the
same way, naming the primitive: `(scroll)` on an `an-view`, `(change)` on an
`an-textarea` — which UIKit reports through a `UITextViewDelegate` this host does
not install, and which the Mac turns down for the same reason.

### Android and Wear OS

| Event | Why not |
|---|---|
| `(hover)` | The pointer is the desktop's. Android does send hover events under a mouse or a stylus, but `[cursor]` has no meaning here. See below. |
| `(crown)`, `(crownIdle)` | On a phone only: there is no wheel. On Wear OS both arrive. |

As on the Apple hosts, an output on a primitive that does not report it is
answered naming the widget the node actually mounted.

### macOS

`(swipeLeft)` and friends work, but **only on views this host owns**. AppKit has
no `NSSwipeGestureRecognizer`; the gesture arrives as `swipeWithEvent:` down the
responder chain, which has to be handled on the class. Bind a swipe directly to
a system control — an `NSButton`, an `NSSlider` — and it warns on subscribe and
says to put it on a wrapping `an-view` instead.

### watchOS

The one host that is not a view hierarchy, and the one with the longest list.

| Event | The reason it gives |
|---|---|
| `(pinch)` | `MagnifyGesture` is `@available(watchOS, unavailable)`, and two fingers do not fit on a 40 mm screen. |
| `(rotation)` | `RotateGesture` is `@available(watchOS, unavailable)`. |
| `(back)` | Outside a `NavigationStack` the watch gives no edge drag, and putting one up would nest SwiftUI's layout inside taffy's. |
| `(refresh)` | On a watch you do not pull a list down to reload it — that is the crown, which already arrives as `(crown)`. |
| `(scroll)` | SwiftUI's `ScrollView` does not publish its offset on watchOS 11, this shell's minimum. |
| `(safeArea)` | A watch app takes the whole screen and the system reserves no margins that could be asked about. |
| `(focus)`, `(blur)` | Not yet: on a watch, focus is the same focus that decides who holds the crown, and two owners would make the crown jump elsewhere while typing. |

### Phones have no pointer

`(hover)` is desktop-only, and so is the `[cursor]` prop that goes with it. A
finger has no shape and nothing hovers before it touches.

Both other host families *could* half-deliver it. UIKit ships
`UIHoverGestureRecognizer` — an iPad trackpad, a mouse turned on through
AssistiveTouch, an Apple Pencil held above the glass — and Android sends
`ACTION_HOVER_ENTER` under a mouse or a stylus. Neither is attached, and the
decision is deliberate: what those report is hardware most of these devices do
not have, so the output would fire on the reviewer's iPad and never on the
user's phone, and `[cursor]`, the other half of the pair, has no meaning on
either. An interface that only answers to a pointer cannot be used with a
finger. So the subscription is refused, once, with that reason, instead of being
dropped without a word.

## The warning that is deliberately not printed

Angular registers an element listener for **every** output that appears in a
template, including ones that are not platform events at all — `onChange`,
`valueChange`. Warning about those would mean a warning on every startup about
something that works perfectly, and a warning that always appears is a warning
nobody reads.

So each host keeps a list of the names the framework does send —
`support::KNOWN_EVENTS` on the Mac, `family::KNOWN_EVENTS` for the three UIKit
families, `KNOWN_EVENTS` in `AnHost` — and warns only about the first kind:
something a template genuinely asked the platform for and this platform does not
give. `scripts/check-platform-gaps.sh` reads the refusals out of those hosts and
fails if a platform page does not name them.
