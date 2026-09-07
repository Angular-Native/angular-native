---
title: "an-text-input"
description: "Un campo de una línea — qué teclado sale, qué dice la tecla de retorno, y las cuatro props que en Android son un mismo entero."
sidebar:
  order: 5
---

`UITextField`, `EditText`, `NSTextField`. El teclado, la barra de autocorrección,
los manejadores de selección, el menú de pegar y el comportamiento de
accesibilidad vienen todos del sistema.

Qué teclado sale no es decoración: un campo de correo con el teclado normal te
obliga a buscar la arroba. En Android `keyboardType`, `autoCapitalize`,
`autoCorrect` y `secureTextEntry` son banderas del **mismo entero**, así que o
llegan las cuatro o no llega ninguna — que es por lo que se comprueban juntas en
`scripts/check-list.sh`.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UITextField` | `EditText` | `NSTextField` | `TextField` |

## Props

| Prop | Tipo | Por defecto | |
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

## Eventos

| Evento | Carga | |
|---|---|---|
| `(valueChange)` | `string` | Paired with `value`, it enables `[(value)]` in the template. |
| `(submit)` | `string` | The keyboard's return key. |

### `[ios]` — lo que tiene UIKit y los demás no

| Clave | Tipo | |
|---|---|---|
| `clearButtonMode` | `'never' | 'whileEditing' | 'always'` | The little cross that empties the field. `UITextField.clearButtonMode`. Android has none: over there the convention is to delete with the keyboard. |
| `borderStyle` | `'none' | 'line' | 'bezel' | 'roundedRect'` | The frame UIKit draws around the field. `UITextField.borderStyle`. On Android an `EditText`'s background comes from the theme, and here it is deliberately taken away so that the template supplies the frame. |

### `[android]` — lo que tiene Android y los demás no

| Clave | Tipo | |
|---|---|---|
| `selectAllOnFocus` | `boolean` | On taking focus, the whole text ends up selected. |
| `cursorVisible` | `boolean` | Hides the caret. `EditText.setCursorVisible`. |

## Ejemplo

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

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
