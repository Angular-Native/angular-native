---
title: "an-modal"
description: "Contenido presentado por encima de todo — y presentado de verdad, que es lo que hace que el lector de pantalla y el botón de volver se comporten."
sidebar:
  order: 18
---

Un `UIViewController` presentado en iOS y un `Dialog` en Android — **no** una
vista puesta encima de las demás. Se parece, y la diferencia importa: el sistema
sabe que hay algo modal delante, así que VoiceOver y TalkBack dejan de leer lo
que hay detrás, el botón de volver de Android lo cierra, y no compite por el
orden de dibujo con los diálogos del propio sistema.

`(dismiss)` se dispara cuando lo cierra el **usuario** —la hoja arrastrada hacia
abajo, el botón de volver— a diferencia de que `[visible]` pase a falso porque lo
haya dicho tu código.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| a presented `UIViewController` | `Dialog` | a presented sheet | `.sheet` |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `visible` | `boolean \| null` | `false` |  |
| `presentation` | `'fullScreen' \| 'sheet' \| null` | `null` | `fullScreen` covers the screen; `sheet` comes up from the bottom with the system's grabber and detents. |

## Eventos

| Evento | Carga | |
|---|---|---|
| `(dismiss)` | — | It closed. |

## Ejemplo

```html
<an-modal [visible]="editing()" [presentation]="'sheet'" (dismiss)="editing.set(false)">
  <an-safe-area [edges]="['top', 'bottom']" [padding]="20" [style.gap]="'12'">
    <an-text [fontSize]="20">Edit note</an-text>
    <an-textarea [value]="draft()" (change)="draft.set($event.value)" />
  </an-safe-area>
</an-modal>
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
