---
title: "an-activity-indicator"
description: "The indeterminate spinner, which hides itself when it stops."
sidebar:
  order: 22
---

`UIActivityIndicatorView`, Android's `ProgressBar` in its indeterminate style,
`NSProgressIndicator` spinning.

It hides itself when `animating` is false — `setHidesWhenStopped(true)` on iOS
and `setDisplayedWhenStopped(false)` on the Mac — so there is no need to wrap it
in an `@if` to keep a stopped spinner off the screen. The space it occupies in
the layout stays reserved, which is usually what you want: the content does not
jump when the load finishes.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIActivityIndicatorView` | `ProgressBar` | `NSProgressIndicator` | `ProgressView` |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `animating` | `boolean \| null` | `true` |  |
| `color` | `string \| null` | `null` |  |

## Example

```html
<an-activity-indicator [animating]="loading()" [color]="'#8a93a6'" />
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
