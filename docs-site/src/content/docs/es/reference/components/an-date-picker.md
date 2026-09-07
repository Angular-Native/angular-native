---
title: "an-date-picker"
description: "Una fecha, una hora o las dos, con lo que ponga cada plataforma para ello."
sidebar:
  order: 13
---

`UIDatePicker` en iOS, `NSDatePicker` en el Mac, un `DatePicker` de SwiftUI en el
reloj. En Android es el **diálogo del sistema**: los selectores de fecha y hora
de Material son diálogos, no controles en línea, y poner uno en línea allí sería
inventarse un control que la plataforma no tiene.

En tvOS no. `value` admite un `Date` o milisegundos; `(change)` reporta
milisegundos.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | — | ✓ | ✓ | — |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIDatePicker` | system dialog | `NSDatePicker` | `DatePicker` |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `value` | `number \| Date \| null` | `null` |  |
| `mode` | `'date' \| 'time' \| 'dateAndTime' \| null` | `'date'` |  |

## Eventos

| Evento | Carga | |
|---|---|---|
| `(change)` | `{ value: number }` |  |

## Ejemplo

```html
<an-date-picker
  [value]="departure()"
  [mode]="'dateAndTime'"
  (change)="departure.set(new Date($event.value))" />
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
