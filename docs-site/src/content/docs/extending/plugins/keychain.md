---
title: "@angular-native/plugin-keychain"
description: "The keychain and the Android keystore, as an angular-native plugin."
sidebar:
  label: keychain
  order: 4
---

Secrets where the system keeps its own: the keychain on Apple platforms, and on
Android an AES key in the key store — which never leaves it — encrypting a file
private to the app. Android has no keychain, and that difference is written up
in [Biometrics and keychain](/extending/biometrics-and-keychain/).

```bash
npm install @angular-native/plugin-keychain
```

## Where it works

| iOS · iPadOS | macOS | watchOS | Android |
|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ |

## The API

| Method | Returns | |
|---|---|---|
| `set(key: string, value: string, options: KeychainOptions = {})` | `Promise<KeychainWrite>` | Stores or replaces a secret. |
| `get(key: string, options: KeychainOptions = {})` | `Promise<KeychainRead>` |  |
| `has(key: string)` | `Promise<boolean>` | Whether anything is stored under that key, without reading it and without asking for biometrics. |
| `remove(key: string)` | `Promise<boolean>` |  |
| `backing()` | `Promise<KeychainBacking>` | What stores the secrets on this particular device. |

Every method is a call across the bridge, so every one returns a promise, and
every failure is a rejection that names what was asked for. See
[Native modules](/reference/native-modules/) for what crosses and what a
rejection looks like.
