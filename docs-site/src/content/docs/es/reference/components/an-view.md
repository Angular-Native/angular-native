---
title: "an-view"
description: "El contenedor a secas — un UIView, un ViewGroup, un NSView — y la demostración de lo que la clase base le da a cada primitiva."
sidebar:
  order: 1
---

Una vista sin nada propio. No añade ninguna prop más allá de la base, que es
precisamente lo que la convierte en el sitio más claro para ver qué te da la
base: fondo, borde, opacidad, las transformaciones, `animate`, el contrato de
accesibilidad, todos los gestos, `(layout)` y `(safeArea)`.

Es además de lo que está hecho casi cualquier layout. Flexbox corre sobre estas.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIView` | `ViewGroup` | `NSView` | `ZStack` |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `enabled` | `boolean \| null` | `true` |  |

## Ejemplo

```html
<an-view [style.flexDirection]="'row'" [style.gap]="'12'" [style.padding]="'16'"
         [backgroundColor]="'#12151a'" [borderRadius]="12">
  <an-view [style.flex]="'1'" [backgroundColor]="'#1e2a4a'"></an-view>
  <an-view [style.width]="'80'" [backgroundColor]="'#2a1e4a'"></an-view>
</an-view>
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
