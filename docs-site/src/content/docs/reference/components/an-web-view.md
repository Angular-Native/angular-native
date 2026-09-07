---
title: "an-web-view"
description: "A real WKWebView or android.webkit.WebView, for the page you genuinely need — and the two platforms that do not have one."
sidebar:
  order: 23
---

`WKWebView` on iOS and the Mac, `android.webkit.WebView` on Android. It takes
either a `[url]` or a string of `[html]`.

There is no irony in a framework that exists to avoid WebViews shipping one: a
terms-of-service page, an OAuth flow or a rich-text article is a document, and a
document is what a web view is for. What it is not for is the interface.

**Not on tvOS**, where WebKit is not in the SDK — and that absence is a `cfg`
that keeps the framework out of the binary, not a stub that fails at runtime.
Not on watchOS either, for the same reason.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | — | ✓ | — | — |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `WKWebView` | `android.webkit.WebView` | `WKWebView` | — |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `url` | `string \| null` | `null` |  |
| `html` | `string \| null` | `null` |  |

## Example

```html
<an-web-view [url]="'https://angular-native.github.io/'" [style.flexGrow]="'1'" />
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
