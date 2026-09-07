---
title: "an-date-picker"
description: "A date, a time or both, through whatever each platform puts up for it."
sidebar:
  order: 13
---

`UIDatePicker` on iOS, `NSDatePicker` on the Mac, a SwiftUI `DatePicker` on the
watch. On Android it is the **system dialog** — Material's date and time pickers
are dialogs, not inline controls, and putting an inline one there would be
inventing a control the platform does not have.

Not on tvOS. `value` takes a `Date` or milliseconds; `(change)` reports
milliseconds.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | — | ✓ | ✓ | — |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIDatePicker` | system dialog | `NSDatePicker` | `DatePicker` |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `value` | `number \| Date \| null` | `null` |  |
| `mode` | `'date' \| 'time' \| 'dateAndTime' \| null` | `'date'` |  |

## Outputs

| Output | Payload | |
|---|---|---|
| `(change)` | `{ value: number }` |  |

## Example

```html
<an-date-picker
  [value]="departure()"
  [mode]="'dateAndTime'"
  (change)="departure.set(new Date($event.value))" />
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
