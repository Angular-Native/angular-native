---
title: "an-map-view"
description: "MKMapView on Apple platforms — and on Android, OpenStreetMap tiles, because the platform ships no map."
sidebar:
  order: 24
---

`MKMapView` on iOS, the Mac, the TV and the headset: the system's map, with its
own gestures, its own labels and its own accessibility.

**On Android it draws OpenStreetMap tiles on a canvas.** Android ships no map:
Google's lives in Play Services behind an API key, which is a dependency this
project will not add on your behalf. So the entry in the support matrix says
"tiles" rather than a tick — the second of only two cells that are neither a yes
nor a no.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | tiles | ✓ | ✓ | ✓ | — | ✓ |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `MKMapView` | OpenStreetMap tiles | `MKMapView` | — |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `latitude` | `number \| null` | `0` |  |
| `longitude` | `number \| null` | `0` |  |
| `zoom` | `number \| null` | `12` | A zoom level in the tile style: 0 is the whole world and each level is twice as close. MapKit does not work that way —it works in how many degrees are on screen— and the host does the conversion, so that the same number means the same thing on both platforms. |
| `showsUser` | `boolean \| null` | `false` | The dot showing where you are. iOS only: Android's map does not know. |

## Example

```html
<an-map-view
  [latitude]="43.3623"
  [longitude]="-8.4115"
  [zoom]="12"
  [showsUser]="true"
  [style.flexGrow]="'1'" />
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
