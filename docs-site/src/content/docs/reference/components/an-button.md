---
title: "an-button"
description: "The system button, with a variant per platform idiom and an icon by name — plus the one prop that needs an explicit height."
sidebar:
  order: 7
---

`UIButton` with its configuration on iOS, `MaterialButton` on Android, `NSButton`
on the Mac. The press feedback, the disabled grey-out, the ripple, the haptic and
the accessibility traits are the control's own.

`[icon]` goes through the same name table as `an-icon`, so `'share'` is an SF
Symbol on one platform and a Material Symbol on the other.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIButton` | `MaterialButton` | `NSButton` | `Button` |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `title` | `string \| null` | `''` |  |
| `color` | `string \| null` | `null` |  |
| `variant` | `'text' \| 'filled' \| 'tonal' \| 'outlined' \| null` | `'text'` | How it looks: the label alone, filled, with a faint background of the same colour, or outlined and empty inside. |
| `icon` | `string \| null` | `null` | An icon beside the label, by name, just like `<an-icon>`. |
| `iconPosition` | `'leading' \| 'trailing' \| null` | `'leading'` | Which side of the label. `leading` by default. |
| `fontSize` | `number \| null` | `null` |  |
| `fontWeight` | `string \| number \| null` | `null` | `'bold'`, `'normal'` or CSS's numeric scale (100..900). |

### `[ios]` — what UIKit has and the others do not

| Key | Type | |
|---|---|---|
| `subtitle` | `string` | A second, smaller line beneath the label. |

### `[android]` — what Android has and the others do not

| Key | Type | |
|---|---|---|
| `rippleColor` | `string` | The colour of the ripple under the finger. `MaterialButton.setRippleColor`. |
| `allCaps` | `boolean` | An upper-case label. It was the norm in Material 2 and stopped being so in Material 3, but it is still there and there are brands that ask for it. On iOS a button has never had its label in upper case. |

:::caution[A subtitle needs a height]
`[ios].subtitle` makes the button two lines tall. The measurement that decides
how big a button is happens **once at startup**, from a sample control with one
line in it, so a button with a subtitle has to be given an explicit height in
the template or the title gets clipped.
:::

## Example

```html
<an-button
  [title]="'Share'"
  [icon]="'share'"
  [iconPosition]="'leading'"
  [variant]="'filled'"
  [enabled]="!busy()"
  (press)="share()" />
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
