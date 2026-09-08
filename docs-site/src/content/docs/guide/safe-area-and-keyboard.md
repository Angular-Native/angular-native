---
title: The safe area and the keyboard
description: One event reports every margin the system reserves — notch, bars, bezel and the keyboard — and it arrives moving, once per frame, so the layout travels with the animation instead of arriving before it.
sidebar:
  order: 9
---

Every platform keeps parts of the screen for itself: a notch, a status bar, a
home indicator, Android's navigation bar, the round bezel of a watch. None of
them are constant and none of them are computable in advance — they change on
rotation, when going into split screen, and when the keyboard comes up.

The host measures them and reports the lot through one event:

```html
<an-view (safeArea)="onInsets($event)"></an-view>
```

```ts
onInsets(insets: NativeSafeAreaInsets) {
  // { top, right, bottom, left }, in points
}
```

Most of the time you do not want the event, you want the padding. That is what
`an-safe-area` is for.

## `<an-safe-area>`

```html
<an-safe-area [edges]="['top', 'bottom']" [padding]="20" [style.gap]="'12'">
  <an-text>no longer sitting under the notch</an-text>
</an-safe-area>
```

```ts
import { SafeArea } from '@angular-native/primitives'
```

| Input | Type | Default |
|---|---|---|
| `edges` | `('top' \| 'right' \| 'bottom' \| 'left')[]` | all four |
| `padding` | `number` | `0` — your own padding, **added on top of** what the system reserves |

Two decisions in it are worth knowing, because both were bugs first.

**There is no view inside it.** The safe area *is* its view, so whatever you put
on it to arrange its children — `gap`, `flexDirection`, `alignItems` — governs
them. With an intermediate wrapper it did not: the styles stayed on the outer
view, which had exactly one child, and nothing happened. No noise, no error, no
spacing.

**`padding` is an input and not `[style.padding]`.** This view's padding is
already written by the safe area; setting both would have them overwrite each
other, and whichever lost would lose in silence.

:::note[Padding, not margin — and not by preference]
The margins the system reserves are measured *for the view they belong to*. A
view that moved itself out of the way with a margin would stop being under the
notch, would therefore start reserving zero, would go back to where it was,
would be under the notch again — and on for ever. Padding does not move the
view, so the measurement stays stable.
:::

## The keyboard is in the bottom inset

It is not a separate event and it is not a sum. While the keyboard is up it is
drawn **over** the home indicator and over the navigation bar, so what arrives in
`bottom` is the larger of the two, not their total.

The practical consequence is short: a form whose last field would otherwise end
up under the keyboard needs nothing but `'bottom'` among its edges.

```html
<an-safe-area [edges]="['top', 'bottom']" [style.flexGrow]="'1'">
  <an-scroll-view [style.flexGrow]="'1'">
    <!-- the last field stays reachable when the keyboard opens -->
  </an-scroll-view>
</an-safe-area>
```

And symmetrically: a safe area that does **not** list `'bottom'` is asking not to
be kept clear of the bottom of the screen, keyboard included. That is a valid
thing to ask for — a full-bleed background, a video — it is just worth knowing
you asked.

## It arrives moving

The inset does not jump to its final value when the keyboard opens. It is
delivered **once per frame**, interpolated to where the keyboard actually is, so
the layout travels with it instead of snapping into place before it.

- **On iOS**, the duration and the curve come out of the keyboard notification,
  which is the same information UIKit's own animations use.
- **On Android**, it is `WindowInsetsAnimation.Callback`, whose `onProgress` is
  called once per frame with the interpolated insets.

Everything the keyboard can do lands there: opened, closed, dragged away with a
finger, resized because the input language changed, and the whole thing again
after a rotation or a split-screen drag.

## The Android gap, stated rather than papered over

`WindowInsets.Type.ime()` does not exist before **API 30**, and neither does
`WindowInsetsAnimation.Callback`. So:

| API level | What arrives |
|---|---|
| 30 and up | The keyboard inset, once per frame, following the animation. |
| 24 – 29 | **Nothing.** A field at the bottom stays under the keyboard. |

The trick that existed before Android R was watching the window's visible frame
shrink, and it only reports anything when the window is allowed to resize. This
shell asks it *not* to resize — `setDecorFitsSystemWindows(false)` — precisely so
that the layout the core computed is the one that gets drawn. Wiring the old
trick back in would mean two layout models running on one screen, which is a
worse thing to own than a documented gap on API levels Play has not accepted an
upload for since 2024.

The bars and the cutout are fine from API 24; it is only the keyboard that is
missing.

## Per platform

| | Where the insets come from |
|---|---|
| **iOS · iPadOS** | `UIView.safeAreaInsets` — notch or Dynamic Island, status bar, home indicator — plus the keyboard, which UIKit does **not** put in `safeAreaInsets` and which this host adds. |
| **tvOS · visionOS** | The same host and the same `safeAreaInsets`, whatever that platform reports for the scene. |
| **Android · Wear OS** | `WindowInsets` — status bar, navigation bar, display cutout, and from API 30 the keyboard. On a round watch this is how the bezel's margin arrives, and it is the difference between a readable list and text clipped by the curve. |
| **macOS** | Zero on all four sides, answered once at subscription time rather than leaving the template waiting. A window's content area is already the content area. |
| **watchOS** | Nothing arrives, and subscribing says so: *"a watch app takes the whole screen and the system reserves no margins that could be asked about"*. `an-safe-area` mounts and stays at zero. |

:::note[The keyboard is not in `safeAreaInsets`]
On iOS those insets are the screen's cut-outs and nothing else — UIKit reports
the keyboard through a notification instead. A framework that only reads
`safeAreaInsets` leaves a field at the bottom of a form sitting under the
keyboard, which is why this host merges the two before reporting.
:::

## Why the first insets can be zero

They arrive **after** the first views have been mounted. Nothing would go back
and ask for them, so the Android shell registers the ordinary
`setOnApplyWindowInsetsListener` as well as the animation callback — without it,
the notch was reported as zero whenever the bundle won the race, which it does
on a loaded machine.

If you are reading `(safeArea)` yourself rather than using `an-safe-area`,
expect the first value to be zeroes and expect a second one shortly after. The
component already handles that: it starts at zero and re-renders when the real
numbers turn up.

## The example

```bash
cargo an dev examples/measure
```

`scripts/check-measure.sh` covers the layout side without a device;
`scripts/check-keyboard-device.sh` is the half that needs a real phone, because
the keyboard's height is the platform's answer and not something a simulator can
be trusted to reproduce.
