---
title: "an-icon"
description: "Un SF Symbol o un Material Symbol por nombre, a un tamaño que además elige el trazo."
sidebar:
  order: 20
---

Treinta y dos nombres comunes se traducen al de cada plataforma, y cualquier otro
pasa tal cual como nombre del símbolo nativo. Aquí no se dibuja nada.

`size` no es solo un tamaño: en las plataformas de Apple un símbolo grande es otro
dibujo, no el pequeño escalado, así que el número va a
`UIImageSymbolConfiguration`. Se escribe además como `width` y `height`, porque el
layout tiene que saber cuánto ocupa la caja.

La tabla completa de nombres, y por qué la fuente de Material va empaquetada y los
SF Symbols no, está en [Iconos](/es/reference/icons/).

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| SF Symbol | Material Symbols | SF Symbol | SF Symbol |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `name` | `string \| null` | `null` |  |
| `size` | `number` | `24` | Points. Besides pinning the view's size down, it picks the symbol's stroke: on iOS a large icon is not the small one scaled up, it is a different drawing. |
| `weight` | `number \| null` | `null` | The stroke's weight, on the typographic scale: 100..900. |
| `color` | `string \| null` | `null` |  |

## Ejemplo

```html
<an-icon [name]="'settings'" [size]="28" [weight]="600" [color]="'#f4f7ff'" />
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
