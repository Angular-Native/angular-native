---
title: "an-stepper"
description: "Más y menos alrededor de un número — y la única primitiva que Android tiene que ensamblar, con piezas de Material."
sidebar:
  order: 10
---

`UIStepper` en iOS, `NSStepper` en el Mac, un `Stepper` de SwiftUI en el reloj.

**En Android está ensamblado, y lo dice.** Material 3 no define ningún stepper,
así que este son dos botones de icono de Material y una etiqueta de Material
— vistas del sistema, juntadas, y no un control dibujado para parecerlo. Esa es
la raya que traza este proyecto: ensamblar con piezas de la plataforma vale,
imitar el dibujo de la plataforma no.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | — | ✓ | ✓ | ✓ |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIStepper` | two icon buttons | `NSStepper` | `Stepper` |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `value` | `number \| null` | `0` |  |
| `minimumValue` | `number \| null` | `0` |  |
| `maximumValue` | `number \| null` | `100` |  |
| `step` | `number \| null` | `1` | How much each tap goes up or down. One by default. |

## Eventos

| Evento | Carga | |
|---|---|---|
| `(change)` | `{ value: number }` |  |

## Ejemplo

```html
<an-stepper
  [value]="quantity()"
  [minimumValue]="1"
  [maximumValue]="10"
  [step]="1"
  (change)="quantity.set($event.value)" />
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
