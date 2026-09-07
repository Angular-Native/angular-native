---
title: "an-text"
description: "Text in the system's real typeface, measured by the platform before the core lays it out."
sidebar:
  order: 2
---

Text is the one leaf the layout cannot guess at. Its size depends on the
typeface, the weight, the letter spacing and the width available, and all of
those live on the platform — so the core asks, through `TextMeasurer`, and the
answer comes from `UILabel`, from `StaticLayout` or from `NSTextField`.

The nine font props look like CSS and are **not** styles: they are props on the
node, because the core needs them to measure and the host needs them to draw.
Writing `[style.fontSize]` works — the renderer reroutes the nine — but
`[fontSize]` is the one the template compiler can check.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UILabel` | `TextView` | `NSTextField` | `Text` |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `color` | `string \| null` | `null` |  |
| `fontSize` | `number \| null` | `null` |  |
| `fontWeight` | `string \| number \| null` | `null` | `'bold'`, `'normal'` or CSS's numeric scale (100..900). |
| `fontStyle` | `'normal' \| 'italic' \| null` | `null` |  |
| `fontFamily` | `string \| null` | `null` |  |
| `letterSpacing` | `number \| null` | `null` |  |
| `lineHeight` | `number \| null` | `null` |  |
| `textAlign` | `'left' \| 'center' \| 'right' \| 'justify' \| null` | `null` |  |
| `numberOfLines` | `number \| null` | `null` | 0 or null = no limit. |
| `textDecoration` | `'none' \| 'underline' \| 'lineThrough' \| null` | `'none'` | Underline or strikethrough. A plain single line, which is what anybody ever asks for. |

### `[android]` — what Android has and the others do not

| Key | Type | |
|---|---|---|
| `selectable` | `boolean` | Lets the text be selected and copied. |

## Example

```html
<an-text [fontSize]="28" [fontWeight]="'600'" [color]="'#f4f7ff'">
  angular-native
</an-text>
<an-text [fontSize]="14" [color]="'#8a93a6'" [numberOfLines]="2">
  Two lines at most; the platform decides where to put the ellipsis.
</an-text>
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
