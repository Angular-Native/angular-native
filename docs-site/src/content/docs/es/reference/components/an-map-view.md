---
title: "an-map-view"
description: "MKMapView en las plataformas de Apple — y en Android, teselas de OpenStreetMap, porque la plataforma no trae mapa."
sidebar:
  order: 24
---

`MKMapView` en iOS, en el Mac, en la tele y en el visor: el mapa del sistema, con
sus gestos, sus etiquetas y su accesibilidad.

**En Android pinta teselas de OpenStreetMap sobre un canvas.** Android no trae
mapa: el de Google vive en Play Services detrás de una clave de API, que es una
dependencia que este proyecto no va a añadir en tu nombre. Así que la casilla de
la matriz dice «teselas» y no un tick — la segunda de las dos únicas que no son
ni un sí ni un no.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | tiles | ✓ | ✓ | ✓ | — | ✓ |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `MKMapView` | OpenStreetMap tiles | `MKMapView` | — |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `latitude` | `number \| null` | `0` |  |
| `longitude` | `number \| null` | `0` |  |
| `zoom` | `number \| null` | `12` | A zoom level in the tile style: 0 is the whole world and each level is twice as close. MapKit does not work that way —it works in how many degrees are on screen— and the host does the conversion, so that the same number means the same thing on both platforms. |
| `showsUser` | `boolean \| null` | `false` | The dot showing where you are. iOS only: Android's map does not know. |

## Ejemplo

```html
<an-map-view
  [latitude]="43.3623"
  [longitude]="-8.4115"
  [zoom]="12"
  [showsUser]="true"
  [style.flexGrow]="'1'" />
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
