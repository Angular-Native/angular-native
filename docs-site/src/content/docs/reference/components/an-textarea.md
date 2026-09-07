---
title: "an-textarea"
description: "Multi-line text entry. Called textarea rather than TextEditor because the an- prefix gave the tag its name back."
sidebar:
  order: 6
---

`UITextView` on iOS, `EditText` with multiple lines on Android, `NSTextView` on
the Mac. Not available on watchOS.

On the wire it still travels as `TextEditor`: the name was chosen when the tag
could not be called `TextArea`, because Angular refuses to self-close anything
named like an HTML element. The prefix later fixed the tag; renaming the core's
enum would mean changing the protocol.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | — | ✓ |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UITextView` | `EditText` | `NSTextView` | — |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `value` | `string \| null` | `''` |  |
| `editable` | `boolean \| null` | `true` |  |
| `color` | `string \| null` | `null` |  |

## Outputs

| Output | Payload | |
|---|---|---|
| `(change)` | `{ value: string }` |  |

## Example

```html
<an-textarea
  [value]="notes()"
  [editable]="!saving()"
  [color]="'#f4f7ff'"
  [style.height]="'160'"
  (change)="notes.set($event.value)" />
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
