---
title: "an-activity-indicator"
description: "El spinner indeterminado, que se esconde solo cuando para."
sidebar:
  order: 22
---

`UIActivityIndicatorView`, el `ProgressBar` indeterminado de Android,
`NSProgressIndicator` girando.

Se esconde solo cuando `animating` es falso —`setHidesWhenStopped(true)` en iOS y
`setDisplayedWhenStopped(false)` en el Mac— así que no hace falta envolverlo en un
`@if` para quitar de la pantalla un spinner parado. El sitio que ocupa en el
layout se sigue reservando, que suele ser lo que quieres: el contenido no da un
salto cuando termina la carga.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIActivityIndicatorView` | `ProgressBar` | `NSProgressIndicator` | `ProgressView` |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `animating` | `boolean \| null` | `true` |  |
| `color` | `string \| null` | `null` |  |

## Ejemplo

```html
<an-activity-indicator [animating]="loading()" [color]="'#8a93a6'" />
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
