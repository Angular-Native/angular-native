---
title: "an-stepper"
description: "Plus and minus around a number — and the one primitive Android has to assemble, out of Material's own parts."
sidebar:
  order: 10
---

`UIStepper` on iOS, `NSStepper` on the Mac, a SwiftUI `Stepper` on the watch.

**On Android it is assembled, and it says so.** Material 3 defines no stepper, so
this one is two Material icon buttons and a Material label — system views, put
together, rather than a control drawn to look like one. That is the line this
project draws: assembling out of the platform's parts is allowed, imitating the
platform's drawing is not.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | — | ✓ | ✓ | ✓ |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIStepper` | two icon buttons | `NSStepper` | `Stepper` |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `value` | `number \| null` | `0` |  |
| `minimumValue` | `number \| null` | `0` |  |
| `maximumValue` | `number \| null` | `100` |  |
| `step` | `number \| null` | `1` | How much each tap goes up or down. One by default. |

## Outputs

| Output | Payload | |
|---|---|---|
| `(change)` | `{ value: number }` |  |

## Example

```html
<an-stepper
  [value]="quantity()"
  [minimumValue]="1"
  [maximumValue]="10"
  [step]="1"
  (change)="quantity.set($event.value)" />
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
