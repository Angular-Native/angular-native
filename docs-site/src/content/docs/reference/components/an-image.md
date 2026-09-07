---
title: "an-image"
description: "A bitmap from a URL or a bundled asset, with the intrinsic size reported back so layout can reserve room before it loads."
sidebar:
  order: 3
---

An image has a size of its own, and the layout needs it before the bytes have
arrived. So `(load)` reports the intrinsic width and height, and the directive
writes them straight back as `intrinsicWidth` and `intrinsicHeight` — two props
the **core** consumes and no host ever sees.

That is what lets a list of remote images stop jumping as they load: the box was
the right size before the picture was there.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIImageView` | `ImageView` | `NSImageView` | `Image` |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `source` | `string \| null` | `null` | The image's path. With no scheme it is a resource in the app's bundle; with `http` or `https` it is fetched over the network and appears when it lands. |
| `resizeMode` | `'contain' \| 'cover' \| 'stretch' \| 'center' \| null` | `null` | `contain` by default; also `cover`, `stretch` and `center`. |
| `intrinsicWidth` | `number \| null` | `null` | The intrinsic size. It fills itself in when the image loads; setting it by hand reserves the space before the image arrives and avoids the jump. |
| `intrinsicHeight` | `number \| null` | `null` |  |

## Outputs

| Output | Payload | |
|---|---|---|
| `(load)` | `{ width, height }` |  |

## Example

```html
<an-image
  [source]="'https://example.com/cover.jpg'"
  [resizeMode]="'cover'"
  [style.width]="'100%'"
  [style.aspectRatio]="'1.5'"
  [borderRadius]="8"
  (load)="onLoad($event)" />
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
