---
title: "an-textarea"
description: "Entrada de texto de varias líneas. Se llama textarea y no TextEditor porque el prefijo an- le devolvió el nombre a la etiqueta."
sidebar:
  order: 6
---

`UITextView` en iOS, `EditText` con varias líneas en Android, `NSTextView` en el
Mac. No está en watchOS.

En el cable sigue viajando como `TextEditor`: el nombre se eligió cuando la
etiqueta no se podía llamar `TextArea`, porque Angular se niega a autocerrar nada
que se llame como un elemento de HTML. El prefijo arregló después la etiqueta;
renombrar el enum del núcleo significaría cambiar el protocolo.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | — | ✓ |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UITextView` | `EditText` | `NSTextView` | — |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `value` | `string \| null` | `''` |  |
| `editable` | `boolean \| null` | `true` |  |
| `color` | `string \| null` | `null` |  |

## Eventos

| Evento | Carga | |
|---|---|---|
| `(change)` | `{ value: string }` |  |

## Ejemplo

```html
<an-textarea
  [value]="notes()"
  [editable]="!saving()"
  [color]="'#f4f7ff'"
  [style.height]="'160'"
  (change)="notes.set($event.value)" />
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
