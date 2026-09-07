---
title: "an-text"
description: "Texto con la tipografía real del sistema, medido por la plataforma antes de que el núcleo lo coloque."
sidebar:
  order: 2
---

El texto es la única hoja que el layout no puede adivinar. Su tamaño depende de
la tipografía, del peso, del espaciado entre letras y del ancho disponible, y
todo eso vive en la plataforma — así que el núcleo pregunta, a través de
`TextMeasurer`, y contesta `UILabel`, `StaticLayout` o `NSTextField`.

Las nueve props de fuente parecen CSS y **no** son estilos: son props del nodo,
porque el núcleo las necesita para medir y el host para pintar. Escribir
`[style.fontSize]` funciona —el renderer reencamina las nueve— pero `[fontSize]`
es la que puede comprobar el compilador de plantillas.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UILabel` | `TextView` | `NSTextField` | `Text` |

## Props

| Prop | Tipo | Por defecto | |
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

### `[android]` — lo que tiene Android y los demás no

| Clave | Tipo | |
|---|---|---|
| `selectable` | `boolean` | Lets the text be selected and copied. |

## Ejemplo

```html
<an-text [fontSize]="28" [fontWeight]="'600'" [color]="'#f4f7ff'">
  angular-native
</an-text>
<an-text [fontSize]="14" [color]="'#8a93a6'" [numberOfLines]="2">
  Two lines at most; the platform decides where to put the ellipsis.
</an-text>
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
