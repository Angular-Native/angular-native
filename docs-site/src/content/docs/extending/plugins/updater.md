---
title: "@angular-native/plugin-updater"
description: "Ship new JavaScript without a store release, with a bad bundle costing one launch — as an angular-native plugin."
sidebar:
  label: updater
  order: 9
---

New JavaScript without a store release. The app is a native binary plus a
`main.js`: the binary can only change through the store, the JavaScript is a
file, and this replaces it.

**A bad bundle costs one launch, not the app.** An installed bundle is on
probation: it runs once, and if your code reaches a point where it is plainly
working and calls `notifyReady()` it is kept. A bundle that never confirms —
because it threw, or hung — is thrown away on the next launch and the packaged
one runs instead.

`http` is refused outright and a `sha256` is checked when given. This file
becomes the code the app runs; over `http` anyone on the path chooses what that
code is.

Both stores allow an app to update its own scripts and both forbid using that to
change what the app *is*. Bug fixes and content are what this is for.

```bash
npm install @angular-native/plugin-updater
```

## Where it works

| iOS · iPadOS | macOS | watchOS | Android |
|:--:|:--:|:--:|:--:|
| ✓ | ✓ | — | ✓ |

Where it does not, the reason is the plugin's own and the build says it
rather than the first call at run time:

**watchOS** — the watch shell loads its bundle from the .app and has no writable place to keep another one that survives an update of the phone app it ships inside. A watch app's JavaScript changes when the app it is embedded in is released.

## The API

| Method | Returns | |
|---|---|---|
| `current()` | `Promise<InstalledBundle>` |  |
| `notifyReady()` | `Promise<void>` | Says this bundle works, so it is kept. |
| `download(request: DownloadRequest)` | `Promise<void>` | Downloads a bundle and installs it for the next launch. |
| `reset()` | `Promise<void>` |  |

Every method is a call across the bridge, so every one returns a promise, and
every failure is a rejection that names what was asked for. See
[Native modules](/reference/native-modules/) for what crosses and what a
rejection looks like.
