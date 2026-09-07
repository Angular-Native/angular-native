---
title: "an-video-view"
description: "The system player with its own controls — AVPlayerViewController, Android's VideoView, AVPlayerView."
sidebar:
  order: 25
---

`AVPlayerViewController` on iOS and the TV, `VideoView` on Android,
`AVPlayerView` on the Mac. The transport controls, the scrubber, the AirPlay
button, the picture-in-picture affordance and the lock-screen integration are the
system's — which is a great deal of behaviour not to have to write.

`[playing]` and `[muted]` are the two things worth driving from a signal.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | — | ✓ |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `AVPlayerViewController` | `VideoView` | `AVPlayerView` | — |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `url` | `string \| null` | `null` |  |
| `playing` | `boolean \| null` | `false` |  |
| `muted` | `boolean \| null` | `false` | iOS only: `VideoView` does not hand over the player inside it. |

## Example

```html
<an-video-view
  [url]="'https://example.com/clip.mp4'"
  [playing]="playing()"
  [muted]="muted()"
  [style.width]="'100%'"
  [style.aspectRatio]="'1.777'" />
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
