---
title: "an-alert"
description: "The system alert or action sheet, with its buttons — presented by the platform, not drawn by us."
sidebar:
  order: 19
---

`UIAlertController` on iOS, `MaterialAlertDialog` on Android, `NSAlert` on the
Mac, SwiftUI's `.alert` on the watch. `[sheet]` switches iOS to an action sheet
instead of a centred alert.

`[buttons]` is a list of titles and `(select)` reports which index was chosen.
The button order, the destructive styling and which one is cancel are the
platform's conventions, which differ between iOS and Android in ways worth not
overriding.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIAlertController` | `MaterialAlertDialog` | `NSAlert` | `.alert` |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `sheet` | `boolean \| null` | `false` | An action sheet instead of a centred dialog. |
| `visible` | `boolean \| null` | `false` |  |
| `title` | `string \| null` | `''` |  |
| `message` | `string \| null` | `''` |  |
| `buttons` | `readonly string[] \| null` | `null` | The buttons' titles, in order. With none, an "OK" shows up. |

## Outputs

| Output | Payload | |
|---|---|---|
| `(select)` | `number` | The index of the button that was pressed. |

## Example

```html
<an-alert
  [visible]="confirming()"
  [title]="'Delete this note?'"
  [message]="'It cannot be undone.'"
  [buttons]="['Cancel', 'Delete']"
  (select)="onChoice($event)" />
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
