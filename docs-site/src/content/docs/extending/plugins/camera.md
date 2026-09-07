---
title: "@angular-native/plugin-camera"
description: "The camera and the photo library, presented by the system, as an angular-native plugin."
sidebar:
  label: camera
  order: 7
---

The camera and the photo library, **presented** rather than embedded:
`UIImagePickerController` and `PHPickerViewController` on iOS, the camera intent
and the photo picker on Android.

Nothing crosses the bridge as bytes. A twelve-megapixel photograph as base64 is
sixteen megabytes of string through a JSON boundary, encoded once and parsed
once; the file is written and its path comes back.

The library needs no permission on iOS 14 or Android 13 and later: those pickers
run outside the app and hand back only what was chosen.

```bash
npm install @angular-native/plugin-camera
```

## Where it works

| iOS · iPadOS | macOS | watchOS | Android |
|:--:|:--:|:--:|:--:|
| ✓ | ✓ | — | ✓ |

Where it does not, the reason is the plugin's own and the build says it
rather than the first call at run time:

**watchOS** — a watch has no camera, and watchOS ships neither UIImagePickerController nor PHPickerViewController. What it can do is ask the paired phone to take one, which is a different feature with a different API.

## The API

| Method | Returns | |
|---|---|---|
| `permission()` | `Promise<CameraPermission>` |  |
| `request()` | `Promise<CameraPermission>` |  |
| `takePhoto(options: PhotoOptions = {})` | `Promise<Photo>` | Opens the camera and resolves with what was taken. |
| `pickPhoto(options: PhotoOptions = {})` | `Promise<Photo>` | Opens the photo library. |

Every method is a call across the bridge, so every one returns a promise, and
every failure is a rejection that names what was asked for. See
[Native modules](/reference/native-modules/) for what crosses and what a
rejection looks like.
