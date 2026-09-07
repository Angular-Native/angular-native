---
title: Gestures and animation
description: The recognisers are the system's, the transforms sit outside layout, and the animation runs on the platform's drawing thread — what that buys you, and what each platform does differently.
sidebar:
  order: 5
---

Two things on this page go together more often than not: a gesture reports where
a finger is, and a transform puts a view there. Both are deliberately cheap —
neither one touches layout — which is what makes it possible to follow a finger
at sixty frames a second with the engine thread busy doing something else.

```ts
@Component({
  selector: 'app-card',
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-view
      [backgroundColor]="'#1e2a4a'"
      [borderRadius]="16"
      [translateX]="x()"
      [animate]="settling() ? 220 : null"
      (pan)="onPan($event)"></an-view>
  `
})
export class Card {
  readonly x = signal(0)
  readonly settling = signal(false)

  onPan(event: NativePanEvent) {
    this.settling.set(event.state !== 'move')
    this.x.set(event.state === 'move' ? event.translationX : 0)
  }
}
```

While the finger is down the view goes exactly where the finger is, with no
animation. When the gesture ends, `animate` comes on and the platform slides it
home. JavaScript is not involved in either of those sixty frames.

## The recognisers belong to the system

Nothing here reimplements a gesture. `(longPress)` fires when
`UILongPressGestureRecognizer` says so, and the delay before it counts is
whatever that version of iOS thinks a long press is. The same gesture on Android
has Android's threshold, and on a Mac it is holding the mouse button down rather
than a finger.

That is a feature and not an oversight. An app whose long press is 400 ms on a
platform where everything else waits 500 ms feels wrong in a way nobody can
name.

| Output | iOS · iPadOS | macOS |
|---|---|---|
| `(press)` | `UITapGestureRecognizer` | `NSClickGestureRecognizer` — a click |
| `(doublePress)` | the same, two taps | the same, two clicks |
| `(longPress)` | `UILongPressGestureRecognizer` | `NSPressGestureRecognizer` — the button held down |
| `(pan)` | `UIPanGestureRecognizer` | `NSPanGestureRecognizer` — dragging with the button down |
| `(pinch)` | `UIPinchGestureRecognizer` | `NSMagnificationGestureRecognizer` — trackpad only |
| `(rotation)` | `UIRotationGestureRecognizer` | `NSRotationGestureRecognizer` — trackpad only |
| `(swipeLeft/Right/Up/Down)` | `UISwipeGestureRecognizer`, one per direction | `swipeWithEvent:` — see below |

## A gesture that nobody listens to does not exist

Every gesture output is an `outputFromObservable` over a **cold** observable: the
recogniser is attached when Angular subscribes, and let go when the view is
destroyed. Angular subscribes an output only if the template binds it, so a view
with no `(press)` on it pays nothing — no recogniser, no target object, no
delegate.

This matters at the scale a list reaches. Twenty thousand rows with a `(press)`
each attach twenty thousand recognisers; twenty thousand rows inside one
container that has the `(press)` attach one.

## The four states, and why `cancel` is not a failure

`(pan)`, `(pinch)` and `(rotation)` carry a `state`:

| `state` | When |
|---|---|
| `begin` | The system has decided the gesture counts. |
| `move` | It is under way. Most events are this one. |
| `end` | The finger lifted and the gesture completed. |
| `cancel` | The system took the gesture away. |

`cancel` arrives when another recogniser wins — dragging inside a list that then
starts scrolling is the everyday case. It is not an error and it is not `end`:
whatever was being moved has to go back where it was, not stay halfway. On iOS
both `Cancelled` and `Failed` arrive as `cancel`, because from the template's
point of view there is nothing to tell apart.

`translation` is measured **from where the finger started**, not from the
previous event. Adding it to the position the view had when the gesture began is
the whole of the arithmetic, with nothing to accumulate and no rounding error
dragged along from every step.

## Transforms do not take part in layout

`translateX`, `translateY`, `scale`, `scaleX`, `scaleY` and `rotate` move what is
drawn. They do not move what was measured: a view translated 200 points to the
right still occupies the place it occupied, and its neighbours do not budge.

That is exactly why they are the ones to reach for while a finger is down —
there is nothing to recompute, so the cost is a matrix on the platform's side.
To move something **and** have its neighbour get out of the way, the layout has
to change instead: `[style.marginLeft]`, an order swap, a `flexGrow`.

`rotate` is in radians, which is what `(rotation)` reports, so the two compose
without a conversion in the middle.

:::note[`[rotate]` commands, `(rotation)` reports]
The gesture output could not also be called `rotate`: a class cannot have two
members with the same name, and `[rotate]` was already the transform. The
difference in the names turned out to be worth keeping on its own.
:::

## `[animate]` is a mode, not a command

```html
<an-view [animate]="200" [animateEasing]="'ease-in-out'" [translateX]="x()"></an-view>
```

`animate` is a number of milliseconds. With it set, **changes** to that view stop
being a jump: moving, scaling, changing the opacity or being relocated by layout
are all interpolated by the platform, on its own drawing thread. It is set once
and holds for every change that comes after. Zero or `null` turns it off.

| Prop | Values | Default |
|---|---|---|
| `animate` | milliseconds | off |
| `animateDelay` | milliseconds | `0` |
| `animateEasing` | `linear`, `ease-in`, `ease-out`, `ease-in-out` | `ease-out` |

`ease-out` is the default because it is what a system animation nearly always
does: leave fast, brake on arrival.

The reason this is a prop and not an API is the thread. A `UIView` animation runs
in the render server; a `ValueAnimator` runs on the Android UI thread. Neither of
them comes back through JavaScript on any of the frames in between, so an
animation stays smooth while the engine thread is compiling a route, resolving a
signal graph, or doing anything else that would drop frames if it were driving
the animation itself.

## What each platform does not have

An event a platform cannot deliver **warns when you subscribe to it**. It does
not fail quietly, and it does not pretend.

**On the TV there is no pinch and no rotation.** The Siri Remote's surface is
single-touch, and `UIPinchGestureRecognizer` and `UIRotationGestureRecognizer`
are not in the tvOS SDK at all. `(pan)` and the four swipes do work: the surface
reports a drag. `(press)` arrives from the centre button, not from a touch —
which means a view that cannot take focus can never be pressed. That is the
platform, not a limitation here, and it is spelled out in
[platforms/tvos](/platforms/tvos/).

**On the desktop the swipe is not a recogniser.** AppKit ships no
`NSSwipeGestureRecognizer`; the gesture exists, but it arrives as
`swipeWithEvent:` travelling up the responder chain, which means it has to be
handled on the view's *class* rather than attached to an arbitrary view. Only the
views this host owns can catch it. Put a `(swipeLeft)` straight onto a system
control — an `NSButton`, an `NSSlider` — and it warns when you subscribe and says
where to put it instead: on a wrapping `an-view`.

It is not imitated with a `pan` and a distance threshold, for the same reason
nothing else here is imitated: the threshold would be ours, and the system's is
the one the user's other apps use.

**On a phone there is no pointer.** `(hover)` and `[cursor]` are desktop-only,
and that is declared rather than forgotten — a finger has no shape and nothing
hovers before it touches. Where there *is* a mouse, both are set up with an
`NSTrackingArea`, which unlike a recogniser does not have to be handled by the
view itself, so they work over a system `NSButton` exactly as they do over an
`an-view`.

## When events are delivered

Never in the middle of a frame. A native event is queued and handed to
JavaScript at the **start of the next frame**, so everything that happened
between two vsyncs is processed in one turn and produces one commit.

A drag therefore gives you one `(pan)` per frame and not one per touch sample,
which is what you want: the extra samples would each cost a change-detection
pass and produce a tree identical to the one the frame is going to mount anyway.

## The example

```bash
cargo an dev examples/gestures
```

`examples/gestures` has each of them on its own view, printing the payload as it
arrives. `scripts/check-gestures.sh` drives the same app through `headless` and
checks the shape of what comes out, so the payloads on this page are the ones
the code produces rather than the ones it was meant to.
