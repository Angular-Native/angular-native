---
title: "an-stack-view"
description: "El contenedor sobre el que se construye una pila de navegación nativa: una pantalla entra deslizándose y la anterior sale por detrás."
sidebar:
  order: 17
---

Un contenedor recortado que anima la transición entre lo que tenía y lo que
tiene ahora. `[transition]` dice en qué dirección —`push`, `pop` o `none`— y
`(back)` es por donde llegan el gesto del borde de iOS y el botón de volver de
Android.

Casi ninguna plantilla lo usa directamente. `<an-native-stack>` lo envuelve
alrededor de un `<router-outlet>` y te conecta la dirección y el volver; ver
[navegación y el router](/es/guide/navigation/).

Está recortado a propósito: sin recortar, las pantallas que entran y salen se
verían deslizándose por encima de todo lo demás.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIView`, clipped | `ViewGroup` | `NSView`, clipped | SwiftUI |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `transition` | `'push' \| 'pop' \| 'none' \| null` | `'none'` | The direction of the next transition. Whoever navigates decides it, being the only one who knows whether this is going forward or back. |

## Eventos

| Evento | Carga | |
|---|---|---|
| `(back)` | — | The edge gesture on iOS, the physical button on Android. |

## Ejemplo

```html
<an-stack-view [style.flexGrow]="'1'" [transition]="direction()" (back)="goBack()">
  <router-outlet />
</an-stack-view>
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
