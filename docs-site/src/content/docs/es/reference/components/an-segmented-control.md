---
title: "an-segmented-control"
description: "Una fila de opciones mutuamente excluyentes — un UISegmentedControl, un MaterialButtonToggleGroup, un NSSegmentedControl."
sidebar:
  order: 11
---

Tres o cuatro opciones, todas visibles a la vez, una seleccionada. Donde un
`an-select` esconde las opciones hasta que lo abres, este las enseña.

En el Mac es además en lo que se convierte `an-tab-bar`, porque un
`NSSegmentedControl` es lo que usan de verdad las apps de Mac para cambiar de
sección.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | — | — |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UISegmentedControl` | `MaterialButtonToggleGroup` | `NSSegmentedControl` | — |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `items` | `readonly string[] \| null` | `null` |  |
| `selectedIndex` | `number \| null` | `0` |  |
| `color` | `string \| null` | `null` |  |

## Eventos

| Evento | Carga | |
|---|---|---|
| `(change)` | `{ index: number }` |  |

## Ejemplo

```html
<an-segmented-control
  [items]="['Day', 'Week', 'Month']"
  [selectedIndex]="range()"
  (change)="range.set($event.index)" />
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
