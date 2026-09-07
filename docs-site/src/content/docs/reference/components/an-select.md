---
title: "an-select"
description: "One option out of a list that drops down — and why it is not called Picker even though that is what it travels as."
sidebar:
  order: 12
---

On iOS it is a button that opens a `UIMenu`: there is no dropdown control in
UIKit, and `UIPickerView` is the full-screen wheel, which is a different thing
and no longer what the system uses for a short list. On Android it is a
`Spinner`, on the Mac an `NSPopUpButton`, on the watch a SwiftUI `Picker`.

It travels to the core as `Picker` — the name it was given when the tag could
not be called `Select`. Renaming the enum now would mean changing the protocol.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | — |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIButton` + `UIMenu` | `Spinner` | `NSPopUpButton` | `Picker` |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `items` | `readonly string[] \| null` | `null` |  |
| `selectedIndex` | `number \| null` | `0` |  |

## Outputs

| Output | Payload | |
|---|---|---|
| `(change)` | `{ index: number }` |  |

## Example

```html
<an-select
  [items]="['Draft', 'In review', 'Published']"
  [selectedIndex]="status()"
  (change)="status.set($event.index)" />
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
