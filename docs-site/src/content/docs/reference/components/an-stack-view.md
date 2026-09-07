---
title: "an-stack-view"
description: "The container a native navigation stack is built on: one screen slides in, the previous one slides out behind it."
sidebar:
  order: 17
---

A clipped container that animates a transition between what it held and what it
holds now. `[transition]` says which direction — `push`, `pop` or `none` — and
`(back)` is where iOS's edge-swipe gesture and Android's back button arrive.

Most templates do not use it directly. `<an-native-stack>` wraps it around a
`<router-outlet>` and wires the direction and the back for you; see
[navigation and the router](/guide/navigation/).

It is clipped on purpose: unclipped, screens on their way in and out would be
seen sliding over everything else.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIView`, clipped | `ViewGroup` | `NSView`, clipped | SwiftUI |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `transition` | `'push' \| 'pop' \| 'none' \| null` | `'none'` | The direction of the next transition. Whoever navigates decides it, being the only one who knows whether this is going forward or back. |

## Outputs

| Output | Payload | |
|---|---|---|
| `(back)` | — | The edge gesture on iOS, the physical button on Android. |

## Example

```html
<an-stack-view [style.flexGrow]="'1'" [transition]="direction()" (back)="goBack()">
  <router-outlet />
</an-stack-view>
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
