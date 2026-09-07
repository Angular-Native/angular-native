---
title: "an-text-input"
description: "A single-line field — which keyboard comes up, what the return key says, and the four props that are one Android integer."
sidebar:
  order: 5
---

`UITextField`, `EditText`, `NSTextField`. The keyboard, the autocorrect bar, the
selection handles, the paste menu and the accessibility behaviour all come from
the system.

Which keyboard comes up is not decoration: an email field with the plain
keyboard makes you hunt for the at sign. On Android `keyboardType`,
`autoCapitalize`, `autoCorrect` and `secureTextEntry` are flags of the **same
integer**, so either all four arrive together or none of them does — which is
why they are checked together in `scripts/check-list.sh`.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UITextField` | `EditText` | `NSTextField` | `TextField` |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `placeholder` | `string \| null` | `null` |  |
| `value` | `string \| null` | `null` | The host only writes into the field when the text really does differ: assigning on every keystroke would send the caret to the end. |
| `secureTextEntry` | `boolean \| null` | `null` |  |
| `editable` | `boolean \| null` | `null` |  |
| `color` | `string \| null` | `null` |  |
| `fontSize` | `number \| null` | `null` |  |
| `fontWeight` | `string \| number \| null` | `null` | `'bold'`, `'normal'` or CSS's numeric scale (100..900). |
| `fontFamily` | `string \| null` | `null` |  |
| `textAlign` | `'left' \| 'center' \| 'right' \| null` | `null` |  |
| `keyboardType` | `'default' \| 'numeric' \| 'decimal' \| 'email' \| 'phone' \| 'url' \| null` | `'default'` | Which keyboard comes up. |
| `returnKeyType` | `'default' \| 'done' \| 'go' \| 'next' \| 'search' \| 'send' \| null` | `'default'` | What the return key says. It changes the label and, with it, what the person expects to happen when they press it. |
| `autoCapitalize` | `'none' \| 'sentences' \| 'words' \| 'characters' \| null` | `'sentences'` |  |
| `autoCorrect` | `boolean \| null` | `true` | The system's autocorrect. Turning it off is the usual thing for a username or a code. |
| `placeholderColor` | `string \| null` | `null` | The placeholder's colour, which does not have to be the text's. |

## Outputs

| Output | Payload | |
|---|---|---|
| `(valueChange)` | `string` | Paired with `value`, it enables `[(value)]` in the template. |
| `(submit)` | `string` | The keyboard's return key. |

### `[ios]` — what UIKit has and the others do not

| Key | Type | |
|---|---|---|
| `clearButtonMode` | `'never' | 'whileEditing' | 'always'` | The little cross that empties the field. `UITextField.clearButtonMode`. Android has none: over there the convention is to delete with the keyboard. |
| `borderStyle` | `'none' | 'line' | 'bezel' | 'roundedRect'` | The frame UIKit draws around the field. `UITextField.borderStyle`. On Android an `EditText`'s background comes from the theme, and here it is deliberately taken away so that the template supplies the frame. |

### `[android]` — what Android has and the others do not

| Key | Type | |
|---|---|---|
| `selectAllOnFocus` | `boolean` | On taking focus, the whole text ends up selected. |
| `cursorVisible` | `boolean` | Hides the caret. `EditText.setCursorVisible`. |

## Example

```html
<an-text-input
  [placeholder]="'Search ships'"
  [value]="query()"
  [keyboardType]="'default'"
  [returnKeyType]="'search'"
  [autoCapitalize]="'none'"
  [autoCorrect]="false"
  (valueChange)="query.set($event)"
  (submit)="run($event)" />
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
