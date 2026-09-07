---
title: "an-select"
description: "Una opción de una lista que se despliega — y por qué no se llama Picker aunque sea como viaja."
sidebar:
  order: 12
---

En iOS es un botón que abre un `UIMenu`: en UIKit no hay control desplegable, y
`UIPickerView` es la rueda a pantalla completa, que es otra cosa y ya no es lo
que usa el sistema para una lista corta. En Android es un `Spinner`, en el Mac un
`NSPopUpButton`, en el reloj un `Picker` de SwiftUI.

Viaja al núcleo como `Picker` — el nombre que se le dio cuando la etiqueta no se
podía llamar `Select`. Renombrar el enum ahora significaría cambiar el
protocolo.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | — |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIButton` + `UIMenu` | `Spinner` | `NSPopUpButton` | `Picker` |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `items` | `readonly string[] \| null` | `null` |  |
| `selectedIndex` | `number \| null` | `0` |  |

## Eventos

| Evento | Carga | |
|---|---|---|
| `(change)` | `{ index: number }` |  |

## Ejemplo

```html
<an-select
  [items]="['Draft', 'In review', 'Published']"
  [selectedIndex]="status()"
  (change)="status.set($event.index)" />
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
