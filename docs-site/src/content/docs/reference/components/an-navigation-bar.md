---
title: "an-navigation-bar"
description: "A header with a title and a back button — and on the Mac, the window's own title bar rather than a second one drawn inside."
sidebar:
  order: 16
---

`UINavigationBar` on iOS, a `Toolbar` on Android.

**On macOS it puts its title in the window's title bar.** A Mac window already
has one; drawing a second header inside the content would be painting two. That
is one of only two entries in the support matrix that is neither a yes nor a no,
and it is a deliberate answer rather than a gap.

Outside an `an-native-stack` it is just a header: `[showsBack]` draws the button
and `(back)` tells you it was pressed, and what that does is yours.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | title bar | ✓ | ✓ | — | — |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UINavigationBar` | `Toolbar` | the window's title bar | — |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `title` | `string \| null` | `''` |  |
| `showsBack` | `boolean \| null` | `false` |  |
| `backTitle` | `string \| null` | `null` | The back button's label. iOS only: on Android the toolbar carries nothing but the arrow, which is what any app on that platform does. |

## Outputs

| Output | Payload | |
|---|---|---|
| `(back)` | — |  |

## Example

```html
<an-navigation-bar
  [title]="ship().name"
  [showsBack]="true"
  [backTitle]="'Fleet'"
  (back)="location.back()" />
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
