---
title: "an-scroll-view"
description: "A scrolling container, with pull-to-refresh and a scroll offset — and the two styles it needs from its parent or it will not scroll at all."
sidebar:
  order: 4
---

The system's scroll view: `UIScrollView`, Android's `ScrollView`, `NSScrollView`.
The momentum, the rubber-banding at the edges, the scrollbar's behaviour and the
way it interacts with the keyboard are the platform's, not an imitation.

The core reports `contentSize` — how much room the children take — and clamps it
to the view's own frame across the axis it is not scrolling on: one axis, never
two. Which one that is is `[horizontal]`'s business.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIScrollView` | `ScrollView` | `NSScrollView` | `ScrollView` |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `horizontal` | `boolean \| null` | `false` | Scrolls sideways instead of downwards, and lays the children out along x unless the template wrote `[style.flexDirection]` itself. |
| `showsScrollIndicator` | `boolean \| null` | `null` |  |
| `scrollEnabled` | `boolean \| null` | `true` | Whether the finger moves the content. |
| `bounces` | `boolean \| null` | `null` | iOS's bounce on reaching the end. |
| `refreshing` | `boolean \| null` | `false` | Whether it is refreshing. Setting it to `false` closes the spinner; the gesture itself opens it, not this prop. |

## Outputs

| Output | Payload | |
|---|---|---|
| `(refresh)` | — | Pull to refresh. |
| `(scroll)` | `{ x, y }` | Emitted on every frame of the scroll. Layout computes the `contentSize` by itself: it is the size the children take up, and the core sends it to the `UIScrollView` whenever it changes. |

### `[ios]` — what UIKit has and the others do not

| Key | Type | |
|---|---|---|
| `pagingEnabled` | `boolean` | Scrolling comes to rest at multiples of the view's size. `UIScrollView.isPagingEnabled`. Android does not ship it: its answer is `ViewPager2`, which is another view with its own adapter, not a prop. |
| `keyboardDismissMode` | `'none' | 'onDrag' | 'interactive'` | What the keyboard does while scrolling. `keyboardDismissMode`. On Android the keyboard does not hide on scroll and there is nothing to ask it for. |

### It has to be allowed to be smaller than its content

A scroll view measured at its content height is not a scroll view — it is a very
tall column, and the page scrolls instead of it. It needs a share of the space
and permission to shrink:

```html
<an-view [style.flexGrow]="'1'">
  <an-scroll-view [style.flexGrow]="'1'" [style.minHeight]="'0'" [style.overflow]="'scroll'">
    …
  </an-scroll-view>
</an-view>
```

This is the single most common reason a scroll view "does nothing".

A `[style.height]` does the same job and is honoured: the `flex-basis` the core
uses to keep the content from sizing the view yields to whatever size the
template asked for on the parent's main axis.

### Sideways

```html
<an-scroll-view [horizontal]="true" [style.height]="'96'">
  @for (card of cards(); track card.id) {
    <an-view [style.width]="'120'">…</an-view>
  }
</an-scroll-view>
```

`[horizontal]` turns the children sideways as well, because a horizontal scroll
view whose children still stack downwards is never wider than itself and so has
nothing to scroll. That is a default: writing `[style.flexDirection]` out takes
the decision back.

It is a boolean and not a `direction` enum because an enum would have to offer
`both`, and `both` is not something Android can be — a `ScrollView` and a
`HorizontalScrollView` are two classes, and a view cannot change class after it
has been made.

## Example

```html
<an-scroll-view
  [style.flexGrow]="'1'"
  [style.overflow]="'scroll'"
  [refreshing]="loading()"
  (refresh)="reload()"
  (scroll)="offset.set($event.y)">
  @for (row of rows(); track row.id) {
    <an-text>{{ row.name }}</an-text>
  }
</an-scroll-view>
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
