---
title: "an-image"
description: "Un mapa de bits desde una URL o un recurso empaquetado, con el tamaño intrínseco reportado de vuelta para que el layout reserve sitio antes de que cargue."
sidebar:
  order: 3
---

Una imagen tiene un tamaño propio, y el layout lo necesita antes de que hayan
llegado los bytes. Así que `(load)` reporta el ancho y el alto intrínsecos, y la
directiva los vuelve a escribir directamente como `intrinsicWidth` e
`intrinsicHeight` — dos props que consume el **núcleo** y que ningún host llega a
ver.

Eso es lo que hace que una lista de imágenes remotas deje de dar saltos según
cargan: la caja ya tenía el tamaño correcto antes de que estuviera la foto.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIImageView` | `ImageView` | `NSImageView` | `Image` |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `source` | `string \| null` | `null` | The image's path. With no scheme it is a resource in the app's bundle; with `http` or `https` it is fetched over the network and appears when it lands. |
| `resizeMode` | `'contain' \| 'cover' \| 'stretch' \| 'center' \| null` | `null` | `contain` by default; also `cover`, `stretch` and `center`. |
| `intrinsicWidth` | `number \| null` | `null` | The intrinsic size. It fills itself in when the image loads; setting it by hand reserves the space before the image arrives and avoids the jump. |
| `intrinsicHeight` | `number \| null` | `null` |  |

## Eventos

| Evento | Carga | |
|---|---|---|
| `(load)` | `{ width, height }` |  |

## Ejemplo

```html
<an-image
  [source]="'https://example.com/cover.jpg'"
  [resizeMode]="'cover'"
  [style.width]="'100%'"
  [style.aspectRatio]="'1.5'"
  [borderRadius]="8"
  (load)="onLoad($event)" />
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
