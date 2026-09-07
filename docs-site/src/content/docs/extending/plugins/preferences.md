---
title: "@angular-native/plugin-preferences"
description: "Small values that outlive the app being closed — UserDefaults and SharedPreferences, as an angular-native plugin."
sidebar:
  label: preferences
  order: 2
---

Small values that outlive the app being closed: `UserDefaults` on the Apple
platforms and `SharedPreferences` on Android — the store every one of them
already has for settings, backed up with the device and readable before the
first frame.

**Strings only, on purpose.** Both stores can hold numbers, booleans, dates and
arrays, and the sets they can hold are not the same set. A key written as a
number on one platform and read as a string on the other is a bug that only
shows up on the platform nobody tested, so the wire carries one type and
anything structured goes through `JSON.stringify`.

**This is not secure storage.** A preferences file is plain text on a device
somebody can root. Tokens and keys belong in the keychain plugin.

```bash
npm install @angular-native/plugin-preferences
```

## Where it works

| iOS · iPadOS | macOS | watchOS | Android |
|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ |

## The API

| Method | Returns | |
|---|---|---|
| `get(key: string)` | `Promise<string \| null>` |  |
| `set(key: string, value: string)` | `Promise<void>` |  |
| `remove(key: string)` | `Promise<void>` |  |
| `keys()` | `Promise<string[]>` | Every key this app has written, in no particular order. |
| `clear()` | `Promise<void>` |  |

Every method is a call across the bridge, so every one returns a promise, and
every failure is a rejection that names what was asked for. See
[Native modules](/reference/native-modules/) for what crosses and what a
rejection looks like.
