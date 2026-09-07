---
title: "an-slider"
description: "A continuous value between two bounds, with the one prop that turns it into a stepped one."
sidebar:
  order: 9
---

`UISlider`, Material's `Slider`, `NSSlider`. Not on tvOS — the nearest thing that
platform ships is something else entirely.

The two platforms disagree about what a slider is for, and both disagreements
are exposed rather than averaged: iOS has `continuous`, which decides whether the
value arrives while dragging or only on release; Android has `stepSize`, which
turns the track into discrete stops with the tick marks Material draws for
them.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | — | ✓ | ✓ | ✓ |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UISlider` | `Slider` | `NSSlider` | `Slider` |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `value` | `number \| null` | `0` |  |
| `minimumValue` | `number \| null` | `0` |  |
| `maximumValue` | `number \| null` | `1` |  |
| `color` | `string \| null` | `null` |  |
| `minimumTrackColor` | `string \| null` | `null` | The stretch already covered, from the left to the thumb. |
| `maximumTrackColor` | `string \| null` | `null` | The stretch still to go. |
| `thumbColor` | `string \| null` | `null` |  |

## Outputs

| Output | Payload | |
|---|---|---|
| `(valueChange)` | `number` |  |

### `[ios]` — what UIKit has and the others do not

| Key | Type | |
|---|---|---|
| `continuous` | `boolean` | Whether it reports while being dragged or only on release. `UISlider.isContinuous`. Material's always reports while being dragged and that cannot be changed. |

### `[android]` — what Android has and the others do not

| Key | Type | |
|---|---|---|
| `stepSize` | `number` | The jump between values. `Slider.setStepSize`. |

## Example

```html
<an-slider
  [value]="volume()"
  [minimumValue]="0"
  [maximumValue]="100"
  [ios]="{ continuous: false }"
  [android]="{ stepSize: 5 }"
  (valueChange)="volume.set($event)" />
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
