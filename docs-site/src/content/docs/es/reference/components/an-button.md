---
title: "an-button"
description: "El botón del sistema, con una variante por idioma de plataforma y un icono por nombre — más la prop que exige un alto explícito."
sidebar:
  order: 7
---

`UIButton` con su configuración en iOS, `MaterialButton` en Android, `NSButton` en
el Mac. La reacción a la pulsación, el gris de deshabilitado, el ripple, el
háptico y los rasgos de accesibilidad son del propio control.

`[icon]` pasa por la misma tabla de nombres que `an-icon`, así que `'share'` es un
SF Symbol en una plataforma y un Material Symbol en la otra.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIButton` | `MaterialButton` | `NSButton` | `Button` |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `title` | `string \| null` | `''` |  |
| `color` | `string \| null` | `null` |  |
| `variant` | `'text' \| 'filled' \| 'tonal' \| 'outlined' \| null` | `'text'` | How it looks: the label alone, filled, with a faint background of the same colour, or outlined and empty inside. |
| `icon` | `string \| null` | `null` | An icon beside the label, by name, just like `<an-icon>`. |
| `iconPosition` | `'leading' \| 'trailing' \| null` | `'leading'` | Which side of the label. `leading` by default. |
| `fontSize` | `number \| null` | `null` |  |
| `fontWeight` | `string \| number \| null` | `null` | `'bold'`, `'normal'` or CSS's numeric scale (100..900). |

### `[ios]` — lo que tiene UIKit y los demás no

| Clave | Tipo | |
|---|---|---|
| `subtitle` | `string` | A second, smaller line beneath the label. |

### `[android]` — lo que tiene Android y los demás no

| Clave | Tipo | |
|---|---|---|
| `rippleColor` | `string` | The colour of the ripple under the finger. `MaterialButton.setRippleColor`. |
| `allCaps` | `boolean` | An upper-case label. It was the norm in Material 2 and stopped being so in Material 3, but it is still there and there are brands that ask for it. On iOS a button has never had its label in upper case. |

:::caution[Un subtítulo necesita alto]
`[ios].subtitle` hace que el botón ocupe dos líneas. La medición que decide cuál
es el tamaño de un botón se hace **una vez al arrancar**, con un control de
muestra de una sola línea, así que a un botón con subtítulo hay que darle un alto
explícito en la plantilla o el título se recorta.
:::

## Ejemplo

```html
<an-button
  [title]="'Share'"
  [icon]="'share'"
  [iconPosition]="'leading'"
  [variant]="'filled'"
  [enabled]="!busy()"
  (press)="share()" />
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
