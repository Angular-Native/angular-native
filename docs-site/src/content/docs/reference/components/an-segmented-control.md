---
title: "an-segmented-control"
description: "A row of mutually exclusive options — a UISegmentedControl, a MaterialButtonToggleGroup, an NSSegmentedControl."
sidebar:
  order: 11
---

Three or four choices, all visible at once, one of them selected. Where a
`an-select` hides the options until you open it, this one shows them.

On the Mac it is also what `an-tab-bar` becomes, because an `NSSegmentedControl`
is what Mac apps actually use to change section.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | — | — |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UISegmentedControl` | `MaterialButtonToggleGroup` | `NSSegmentedControl` | — |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `items` | `readonly string[] \| null` | `null` |  |
| `selectedIndex` | `number \| null` | `0` |  |
| `color` | `string \| null` | `null` |  |

## Outputs

| Output | Payload | |
|---|---|---|
| `(change)` | `{ index: number }` |  |

## Example

```html
<an-segmented-control
  [items]="['Day', 'Week', 'Month']"
  [selectedIndex]="range()"
  (change)="range.set($event.index)" />
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
