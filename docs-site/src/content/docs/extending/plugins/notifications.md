---
title: "@angular-native/plugin-notifications"
description: "Local notifications, and the token for remote ones — UNUserNotificationCenter and NotificationManager, as an angular-native plugin."
sidebar:
  label: notifications
  order: 8
---

Notifications the app schedules itself. `UNUserNotificationCenter` on the Apple
platforms, and on Android `NotificationManager` with an `AlarmManager` behind it
— a process that has been killed cannot post anything, so a scheduled
notification belongs to the system and not to the app.

The tap that **launched** the app is the one worth not losing, and it is the
hardest: the system delivers it before JS exists. Nothing native can ask whether
anybody is listening yet, so taps are held until JS subscribes for the first
time, and emitted live from then on.

`remoteToken()` rejects and says why. APNs needs the shell to register and hand
the token over; FCM needs a Firebase project this shell does not bundle.

```bash
npm install @angular-native/plugin-notifications
```

## Where it works

| iOS · iPadOS | macOS | watchOS | Android |
|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ |

## The API

| Method | Returns | |
|---|---|---|
| `permission()` | `Promise<NotificationPermission>` |  |
| `request()` | `Promise<NotificationPermission>` | Asks, if there is anything to ask, and waits for the answer. |
| `schedule(notification: LocalNotification)` | `Promise<void>` |  |
| `cancel(id: string)` | `Promise<void>` |  |
| `pending()` | `Promise<string[]>` |  |
| `clearDelivered()` | `Promise<void>` |  |
| `remoteToken()` | `Promise<string>` | The token this device is reachable at, for remote notifications. |

Every method is a call across the bridge, so every one returns a promise, and
every failure is a rejection that names what was asked for. See
[Native modules](/reference/native-modules/) for what crosses and what a
rejection looks like.
