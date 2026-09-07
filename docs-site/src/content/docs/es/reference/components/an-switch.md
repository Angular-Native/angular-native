---
title: "an-switch"
description: "El interruptor del sistema — el control por el que existe todo este proyecto."
sidebar:
  order: 8
---

Un `UISwitch` en iOS, un `MaterialSwitch` en Android, un `NSSwitch` en el Mac, un
`Toggle` de SwiftUI en el reloj. En tvOS no: una tele se maneja con un mando y no
hay interruptor en el SDK.

Este es el ejemplo que usa la portada, porque es donde una imitación dibujada se
nota más. Un `UISwitch` de verdad se trae la animación del sistema, el háptico que
la acompaña, el rasgo de accesibilidad que hace que VoiceOver lo llame
interruptor, y el aspecto que tenga en la siguiente versión del sistema.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | — | ✓ | ✓ | ✓ |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UISwitch` | `MaterialSwitch` | `NSSwitch` | `Toggle` |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `on` | `boolean \| null` | `false` |  |
| `color` | `string \| null` | `null` | The colour when it is on. |
| `thumbColor` | `string \| null` | `null` | The thumb's colour, the part that moves. |

## Eventos

| Evento | Carga | |
|---|---|---|
| `(onChange)` | `boolean` | Paired with `on`, it enables `[(on)]` in the template. |

### `[android]` — lo que tiene Android y los demás no

| Clave | Tipo | |
|---|---|---|
| `trackColor` | `string` | The track's colour when the switch is off. |

## Ejemplo

```html
<an-switch
  [on]="alerts()"
  [color]="'#ff5a1f'"
  [enabled]="!locked()"
  (onChange)="alerts.set($event)" />
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
