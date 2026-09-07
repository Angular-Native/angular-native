---
title: "@angular-native/plugin-clipboard"
description: "The system clipboard, as an angular-native plugin."
sidebar:
  label: clipboard
  order: 3
---

The system clipboard: `UIPasteboard` on iOS, `ClipboardManager` on Android,
`NSPasteboard` on the Mac. It is also the reference plugin — the smallest
complete example of the contract, written up end to end on the
[plugins page](/extending/plugins/).

```bash
npm install @angular-native/plugin-clipboard
```

## Where it works

| iOS · iPadOS | macOS | watchOS | Android |
|:--:|:--:|:--:|:--:|
| ✓ | ✓ | — | ✓ |

Where it does not, the reason is the plugin's own and the build says it
rather than the first call at run time:

**watchOS** — watchOS has no system pasteboard: UIPasteboard is API_UNAVAILABLE(watchos) and there is nothing standing in for it. A watch shares text by handing it to the paired phone, which is a different feature with a different API, not a clipboard.

## The API

| Method | Returns | |
|---|---|---|
| `write(text: string)` | `Promise<void>` |  |
| `read()` | `Promise<string>` | Whatever has been copied, or an empty string if there is no text. |
| `hasText()` | `Promise<boolean>` |  |

Every method is a call across the bridge, so every one returns a promise, and
every failure is a rejection that names what was asked for. See
[Native modules](/reference/native-modules/) for what crosses and what a
rejection looks like.
