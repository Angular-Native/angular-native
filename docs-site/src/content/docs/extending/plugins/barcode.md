---
title: "@angular-native/plugin-barcode"
description: "Reading a barcode with the camera, full screen, through AVFoundation — as an angular-native plugin."
sidebar:
  label: barcode
  order: 10
---

Reading a barcode with the camera. `AVCaptureMetadataOutput` decodes on the
device, with no model to download and no service behind it.

It comes two ways. `scan()` opens full screen and closes when something is read.
And there is a **live preview you can put in your own layout**, which is the one
view any plugin in this project contributes — see
[a view a plugin brings](/extending/plugins/#a-view-a-plugin-brings):

```html
<an-custom [view]="'barcode-preview'" [style.height]="'260'" [borderRadius]="16" />
```

The preview emits what it reads on the module event channel rather than
resolving a promise, because a preview produces none or a hundred and a promise
carries one.

```bash
npm install @angular-native/plugin-barcode
```

## Where it works

| iOS · iPadOS | macOS | watchOS | Android |
|:--:|:--:|:--:|:--:|
| ✓ | ✓ | — | — |

Where it does not, the reason is the plugin's own and the build says it
rather than the first call at run time:

**watchOS** — a watch has no camera. AVFoundation's capture classes are not in the watchOS SDK at all.

**Android** — Android ships no barcode API. The usual answer is ML Kit, which lives in Google Play Services and is a Gradle dependency; this toolchain has no Gradle, and bundling the Play-backed variant would make an app that cannot be installed on a device without Play Services. The alternative is embedding a decoder such as ZXing in the shell, which is a decision about what every angular-native app carries and not one a plugin can make on its own.

## The API

| Method | Returns | |
|---|---|---|
| `available()` | `Promise<boolean>` |  |
| `scan(options: ScanOptions = {})` | `Promise<Barcode>` | Opens the scanner and resolves with the first thing it reads. |

Every method is a call across the bridge, so every one returns a promise, and
every failure is a rejection that names what was asked for. See
[Native modules](/reference/native-modules/) for what crosses and what a
rejection looks like.
