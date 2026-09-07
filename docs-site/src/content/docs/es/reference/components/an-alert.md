---
title: "an-alert"
description: "La alerta o la hoja de acciones del sistema, con sus botones — presentada por la plataforma, no dibujada por nosotros."
sidebar:
  order: 19
---

`UIAlertController` en iOS, `MaterialAlertDialog` en Android, `NSAlert` en el
Mac, el `.alert` de SwiftUI en el reloj. `[sheet]` cambia iOS a una hoja de
acciones en vez de una alerta centrada.

`[buttons]` es una lista de títulos y `(select)` reporta qué índice se ha
elegido. El orden de los botones, el estilo destructivo y cuál es el de cancelar
son convenciones de la plataforma, que difieren entre iOS y Android de formas que
conviene no pisar.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIAlertController` | `MaterialAlertDialog` | `NSAlert` | `.alert` |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `sheet` | `boolean \| null` | `false` | An action sheet instead of a centred dialog. |
| `visible` | `boolean \| null` | `false` |  |
| `title` | `string \| null` | `''` |  |
| `message` | `string \| null` | `''` |  |
| `buttons` | `readonly string[] \| null` | `null` | The buttons' titles, in order. With none, an "OK" shows up. |

## Eventos

| Evento | Carga | |
|---|---|---|
| `(select)` | `number` | The index of the button that was pressed. |

## Ejemplo

```html
<an-alert
  [visible]="confirming()"
  [title]="'Delete this note?'"
  [message]="'It cannot be undone.'"
  [buttons]="['Cancel', 'Delete']"
  (select)="onChoice($event)" />
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
