---
title: "an-progress-bar"
description: "Determinate progress, from zero to one."
sidebar:
  order: 21
---

`UIProgressView`, Material's `LinearProgressIndicator`, `NSProgressIndicator`, a
SwiftUI `ProgressView`.

`progress` is a fraction between `0` and `1`, not a percentage. For work whose
length is unknown, `an-activity-indicator` is the right control — a progress bar
that does not progress is worse than a spinner.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIProgressView` | `LinearProgressIndicator` | `NSProgressIndicator` | `ProgressView` |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `progress` | `number \| null` | `0` |  |
| `color` | `string \| null` | `null` |  |

## Example

```html
<an-progress-bar [progress]="uploaded() / total()" [color]="'#ff5a1f'" />
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
