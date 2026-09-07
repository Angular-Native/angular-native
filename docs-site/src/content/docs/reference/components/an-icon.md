---
title: "an-icon"
description: "An SF Symbol or a Material Symbol by name, at a size that also picks the stroke."
sidebar:
  order: 20
---

Thirty-two common names are translated into each platform's own, and anything
else passes through as the native symbol's name. Nothing is drawn here.

`size` is not only a size: on Apple platforms a large symbol is a different
drawing, not the small one scaled up, so the number goes to
`UIImageSymbolConfiguration`. It is also written out as `width` and `height`,
because layout has to know how big the box is.

The full name table, and why the Material font is bundled while SF Symbols are
not, is in [Icons](/reference/icons/).

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| SF Symbol | Material Symbols | SF Symbol | SF Symbol |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `name` | `string \| null` | `null` |  |
| `size` | `number` | `24` | Points. Besides pinning the view's size down, it picks the symbol's stroke: on iOS a large icon is not the small one scaled up, it is a different drawing. |
| `weight` | `number \| null` | `null` | The stroke's weight, on the typographic scale: 100..900. |
| `color` | `string \| null` | `null` |  |

## Example

```html
<an-icon [name]="'settings'" [size]="28" [weight]="600" [color]="'#f4f7ff'" />
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
