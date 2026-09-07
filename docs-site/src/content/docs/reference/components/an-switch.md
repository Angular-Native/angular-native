---
title: "an-switch"
description: "The system switch — the control this whole project exists to argue about."
sidebar:
  order: 8
---

A `UISwitch` on iOS, a `MaterialSwitch` on Android, an `NSSwitch` on the Mac, a
SwiftUI `Toggle` on the watch. Not on tvOS: a television is driven with a remote
and there is no switch in the SDK.

This is the example the front page uses, because it is the one where a drawn
lookalike is most obviously wrong. A real `UISwitch` picks up the system's
animation, the haptic that goes with it, the accessibility trait that makes
VoiceOver call it a switch, and whatever it looks like in the next OS version.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | — | ✓ | ✓ | ✓ |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UISwitch` | `MaterialSwitch` | `NSSwitch` | `Toggle` |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `on` | `boolean \| null` | `false` |  |
| `color` | `string \| null` | `null` | The colour when it is on. |
| `thumbColor` | `string \| null` | `null` | The thumb's colour, the part that moves. |

## Outputs

| Output | Payload | |
|---|---|---|
| `(onChange)` | `boolean` | Paired with `on`, it enables `[(on)]` in the template. |

### `[android]` — what Android has and the others do not

| Key | Type | |
|---|---|---|
| `trackColor` | `string` | The track's colour when the switch is off. |

## Example

```html
<an-switch
  [on]="alerts()"
  [color]="'#ff5a1f'"
  [enabled]="!locked()"
  (onChange)="alerts.set($event)" />
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
