---
title: "@angular-native/plugin-geolocation"
description: "Where the device is — CLLocationManager and Android's LocationManager, as an angular-native plugin."
sidebar:
  label: geolocation
  order: 6
---

Where the device is: `CLLocationManager` on Apple platforms and the platform's
own `LocationManager` on Android — **not** Google's fused provider, which lives
in Play Services and would make an app that cannot be installed on a device
without it.

`current()` is one fix. `watch()` is a stream, and it is not `current()` in a
loop: the platform decides when there is a new position worth reporting, and a
poll would either miss movements or keep the GPS on for nothing.

`watch({ background: true })` keeps going once the app is off screen. On Android
that means a foreground service and **a notification the person can see for as
long as it runs** — the platform requires it precisely so that an app cannot
follow somebody quietly.

```bash
npm install @angular-native/plugin-geolocation
```

## Where it works

| iOS · iPadOS | macOS | watchOS | Android |
|:--:|:--:|:--:|:--:|
| ✓ | ✓ | — | ✓ |

Where it does not, the reason is the plugin's own and the build says it
rather than the first call at run time:

**watchOS** — watchOS does have CoreLocation, but a watch app that wants a fix has to declare a background mode and hold a session open: that is a different feature with a different lifecycle, not the same call on a smaller screen. A watch app that needs a position should ask the paired phone for it.

## The API

| Method | Returns | |
|---|---|---|
| `permission()` | `Promise<LocationPermission>` |  |
| `request()` | `Promise<LocationPermission>` | Asks, if there is anything to ask, and waits for the answer. |
| `current(timeoutMs = 10_000)` | `Promise<Position>` | One fix. |

Every method is a call across the bridge, so every one returns a promise, and
every failure is a rejection that names what was asked for. See
[Native modules](/reference/native-modules/) for what crosses and what a
rejection looks like.
