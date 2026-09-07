---
title: "an-navigation-bar"
description: "Una cabecera con título y botón de volver — y en el Mac, la barra de título de la ventana en vez de una segunda dibujada dentro."
sidebar:
  order: 16
---

`UINavigationBar` en iOS, un `Toolbar` en Android.

**En macOS pone su título en la barra de título de la ventana.** Una ventana de
Mac ya tiene una; dibujar una segunda cabecera dentro del contenido sería pintar
dos. Es una de las dos únicas casillas de la matriz de soporte que no son ni un
sí ni un no, y es una respuesta deliberada, no un hueco.

Fuera de un `an-native-stack` es solo una cabecera: `[showsBack]` pinta el botón
y `(back)` te dice que lo han pulsado, y lo que eso haga es cosa tuya.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | title bar | ✓ | ✓ | — | — |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UINavigationBar` | `Toolbar` | the window's title bar | — |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `title` | `string \| null` | `''` |  |
| `showsBack` | `boolean \| null` | `false` |  |
| `backTitle` | `string \| null` | `null` | The back button's label. iOS only: on Android the toolbar carries nothing but the arrow, which is what any app on that platform does. |

## Eventos

| Evento | Carga | |
|---|---|---|
| `(back)` | — |  |

## Ejemplo

```html
<an-navigation-bar
  [title]="ship().name"
  [showsBack]="true"
  [backTitle]="'Fleet'"
  (back)="location.back()" />
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
