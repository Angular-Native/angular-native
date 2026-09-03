---
title: Built-in modules
description: Files, share, network status and haptics — what each one becomes on each of the eight platforms, and what the ones that cannot have it say instead.
sidebar:
  order: 5
---

A [native module](/reference/native-modules/) is a method JS can call. A
[plugin](/extending/plugins/) is one that arrives as an npm package. A
**built-in** is one the framework brings: nobody declares it, there is no
dependency to add, and every host has it.

There are five. [`Device`](/reference/native-modules/) answers from the engine's
thread because what it says never changes. The other four cannot: a share sheet,
a file picker, a vibrator and a network monitor all live on the UI thread, so
they travel out through a queue and answer when the platform gets to them —
which is the same road a plugin's call takes, with a mailbox of its own so that
nothing installed from npm can stand in front of a name the framework promises.

```ts
import { inject } from '@angular/core'
import { Files, Haptics, Network, Share } from '@angular-native/platform'

export class Draft {
  private readonly files = inject(Files)
  private readonly sharing = inject(Share)
  private readonly haptics = inject(Haptics)

  async save(text: string): Promise<void> {
    await this.files.write('draft.txt', text)
    await this.haptics.notification('success')
  }

  async send(): Promise<void> {
    const home = await this.files.documentsDirectory()
    await this.sharing.share({ title: 'Draft', files: [`${home}/draft.txt`] })
  }
}
```

## What each one is, per platform

| | iOS · iPadOS | tvOS | visionOS | macOS | watchOS | Android | Wear OS |
|---|---|---|---|---|---|---|---|
| **files** — container | `Documents` | *cache only* | `Documents` | `Application Support` | `Documents` | `getFilesDir()` | `getFilesDir()` |
| **files** — picker | `UIDocumentPickerViewController` | **none** | `UIDocumentPickerViewController` | `NSOpenPanel` | **none** | `ACTION_OPEN_DOCUMENT` | **none** |
| **share** | `UIActivityViewController` | **none** | `UIActivityViewController` | `NSSharingServicePicker` | **none** | `ACTION_SEND` chooser | `ACTION_SEND` chooser¹ |
| **network** | `NWPathMonitor` | `NWPathMonitor` | `NWPathMonitor` | `NWPathMonitor` | `NWPathMonitor` | `NetworkCallback` | `NetworkCallback` |
| **haptics** | `UIImpactFeedbackGenerator` | **none** | **none** | `NSHapticFeedbackManager`² | `WKInterfaceDevice.play` | `VibrationEffect` | `VibrationEffect` |

¹ A Wear watch resolves `ACTION_SEND` to Bluetooth and to whatever the
manufacturer added, so the chooser is real but may hold one entry or none. Ask
`canShare()`: on this platform it is the watch answering, not the platform.

² Only if this Mac has a Force Touch trackpad, and AppKit offers no way to ask
whether it does. See `support().caveat`.

## What a platform without it says

Nothing is emulated and nothing quietly does nothing. Where the hardware or the
class is not there, the promise rejects with a sentence that names the platform,
says what is missing and says what to do instead:

- **`files.documentsDirectory()` on tvOS** — a television gives an app no
  storage that lasts; the system may empty the container at any time. It points
  at `cacheDirectory()`, which is where a relative path lands there.
- **`files.pick()` on tvOS, watchOS and Wear OS** — none of the three ships a
  document browser, and on Wear `ACTION_OPEN_DOCUMENT` resolves to nothing.
- **`share.share()` on tvOS and watchOS** — no `UIActivityViewController`, no
  AirDrop, nothing to hand anything to.
- **`haptics.*` on tvOS and visionOS** — a television has nothing to vibrate and
  the Siri Remote has no engine an app can drive; in the headset nothing is held
  and nothing touches the wrist.
- **`haptics.notification()` on macOS** — `NSHapticFeedbackManager` has three
  patterns and none of them means success, warning or error, so playing one for
  all three would be three meanings coming out as one feeling.

Every one of these is also in the types, so an app can be written against it
rather than finding out by catching:

```ts
import type {
  FilePickerPlatform, // every platform but tvos, watchos and wearos
  HapticsPlatform,    // every platform but tvos and visionos
  NetworkPlatform,    // every platform: this one has no exceptions
  SharePlatform       // every platform but tvos and watchos
} from '@angular-native/platform'
```

And where the answer depends on the device rather than the platform, there is a
method to ask: `share.canShare()` and `haptics.support()`.

## Where files may go

`files` draws one line and enforces it rather than documenting it:

- It **writes** only inside `documentsDirectory()` and `cacheDirectory()`.
- Outside those it **reads** only what came back from `pick()`, and only while
  the app is running.

Anything else is rejected saying which of the two it failed. Without that rule, a
path arriving from JS is a path to anywhere on the disk. A relative path is
resolved against `documentsDirectory()`, so `read('notes.txt')` means something
without the app having to ask where that is.

On Android, what `pick()` gives back is a `content://` URI and not a path: that
is what the Storage Access Framework hands over, the document may not be on the
device at all, and a `/`-path invented for it would be a path to nothing. Pass it
straight back to `read()` or to `share()`.

## Permissions

Two of the four need something in the Android manifest, and both are
**install-time**: there is no dialog and nothing the person grants later, so an
app whose manifest lacks the line has a module that can only ever fail.

| Module | Permission | Without it |
|---|---|---|
| `network` | `android.permission.ACCESS_NETWORK_STATE` | `status()` rejects with the line to paste |
| `haptics` | `android.permission.VIBRATE` | every method rejects with the line to paste |

The framework's own shell declares both, so an app built with `an android` has
them. Each module also checks before it calls, which is the difference between
being told what is missing and dying with a `SecurityException` at the first tap.

Sharing a file needs one more thing, and it is not a permission: since Android 7
a `file://` path cannot cross to another app. The shell declares a
`FileProvider` over the two directories `files` owns, and `share` wraps what it
sends. An app whose manifest has no such provider gets the `<provider>` block to
paste rather than a crash.

Nothing on Apple's platforms needs a usage string for these four. A sandboxed Mac
app that wants `files.pick()` needs
`com.apple.security.files.user-selected.read-only` in its entitlements; the
shell here is not sandboxed, so it does not.

## The network signal

`Network` is the one with something to watch:

```ts
const network = inject(Network)
network.watch()               // keeps `network.status()` fresh
network.status()              // Signal<NetworkStatus | null>
```

The monitor underneath is event-driven and always right — it is the system
pushing at the moment the Wi-Fi drops. What is missing is the last hop: there is
no channel from a native module back into JS yet, only answers to calls, so
`watch()` asks it on an interval. The monitoring is the platform's and the
polling is ours, and it is said here rather than dressed up as a subscription.
When that channel exists, `watch()` keeps its shape and loses the timer.

## Trying them

`examples/modules` calls all four and prints a line per answer, including the
refusals:

```bash
cargo an macos examples/modules
cargo an android examples/modules
cargo an tvos examples/modules      # to see what a television refuses
cargo an watchos examples/modules   # and what a watch does
```

`scripts/check-builtins.sh` reads those lines out of the running macOS app, and
`scripts/check-builtins-device.sh` does the same over `adb` on a phone or a
watch — which is the only way to find out what a given Wear device can actually
receive.
