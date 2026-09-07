---
title: "an-modal"
description: "Content presented over everything — and it really is presented, which is what makes the screen reader and the back button behave."
sidebar:
  order: 18
---

A `UIViewController` presented on iOS and a `Dialog` on Android — **not** a view
laid over the others. It looks similar, and the difference matters: the system
knows there is something modal in front, so VoiceOver and TalkBack stop reading
what is behind it, Android's back button closes it, and it does not compete for
draw order with the system's own dialogs.

`(dismiss)` fires when the **user** closes it — the sheet dragged down, the back
button — as opposed to `[visible]` going false because your code said so.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| a presented `UIViewController` | `Dialog` | a presented sheet | `.sheet` |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `visible` | `boolean \| null` | `false` |  |
| `presentation` | `'fullScreen' \| 'sheet' \| null` | `null` | `fullScreen` covers the screen; `sheet` comes up from the bottom with the system's grabber and detents. |

## Outputs

| Output | Payload | |
|---|---|---|
| `(dismiss)` | — | It closed. |

## Example

```html
<an-modal [visible]="editing()" [presentation]="'sheet'" (dismiss)="editing.set(false)">
  <an-safe-area [edges]="['top', 'bottom']" [padding]="20" [style.gap]="'12'">
    <an-text [fontSize]="20">Edit note</an-text>
    <an-textarea [value]="draft()" (change)="draft.set($event.value)" />
  </an-safe-area>
</an-modal>
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
