---
title: "an-video-view"
description: "El reproductor del sistema con sus propios controles — AVPlayerViewController, el VideoView de Android, AVPlayerView."
sidebar:
  order: 25
---

`AVPlayerViewController` en iOS y en la tele, `VideoView` en Android,
`AVPlayerView` en el Mac. Los controles de reproducción, la barra de progreso, el
botón de AirPlay, el picture-in-picture y la integración con la pantalla de
bloqueo son del sistema — que es un montón de comportamiento que no hay que
escribir.

`[playing]` y `[muted]` son las dos cosas que merece la pena llevar desde una
señal.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | — | ✓ |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `AVPlayerViewController` | `VideoView` | `AVPlayerView` | — |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `url` | `string \| null` | `null` |  |
| `playing` | `boolean \| null` | `false` |  |
| `muted` | `boolean \| null` | `false` | iOS only: `VideoView` does not hand over the player inside it. |

## Ejemplo

```html
<an-video-view
  [url]="'https://example.com/clip.mp4'"
  [playing]="playing()"
  [muted]="muted()"
  [style.width]="'100%'"
  [style.aspectRatio]="'1.777'" />
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
