---
title: "an-view"
description: "The plain container — a UIView, a ViewGroup, an NSView — and the demonstration of what every primitive gets from the base class."
sidebar:
  order: 1
---

A view with nothing of its own. It adds no props beyond the base, which is
exactly what makes it the clearest place to see what the base gives you:
background, border, opacity, the transforms, `animate`, the accessibility
contract, every gesture, `(layout)` and `(safeArea)`.

It is also what almost every layout is made of. Flexbox runs over these.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIView` | `ViewGroup` | `NSView` | `ZStack` |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `enabled` | `boolean \| null` | `true` |  |

## Example

```html
<an-view [style.flexDirection]="'row'" [style.gap]="'12'" [style.padding]="'16'"
         [backgroundColor]="'#12151a'" [borderRadius]="12">
  <an-view [style.flex]="'1'" [backgroundColor]="'#1e2a4a'"></an-view>
  <an-view [style.width]="'80'" [backgroundColor]="'#2a1e4a'"></an-view>
</an-view>
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
