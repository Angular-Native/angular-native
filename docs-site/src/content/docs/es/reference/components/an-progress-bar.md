---
title: "an-progress-bar"
description: "Progreso determinado, de cero a uno."
sidebar:
  order: 21
---

`UIProgressView`, el `LinearProgressIndicator` de Material, `NSProgressIndicator`,
un `ProgressView` de SwiftUI.

`progress` es una fracción entre `0` y `1`, no un porcentaje. Para trabajo de
duración desconocida el control correcto es `an-activity-indicator`: una barra de
progreso que no progresa es peor que un spinner.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIProgressView` | `LinearProgressIndicator` | `NSProgressIndicator` | `ProgressView` |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `progress` | `number \| null` | `0` |  |
| `color` | `string \| null` | `null` |  |

## Ejemplo

```html
<an-progress-bar [progress]="uploaded() / total()" [color]="'#ff5a1f'" />
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
