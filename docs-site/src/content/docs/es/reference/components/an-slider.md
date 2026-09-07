---
title: "an-slider"
description: "Un valor continuo entre dos límites, con la prop que lo convierte en escalonado."
sidebar:
  order: 9
---

`UISlider`, el `Slider` de Material, `NSSlider`. En tvOS no: lo más parecido que
trae esa plataforma es otra cosa.

Las dos plataformas no están de acuerdo en para qué sirve un slider, y los dos
desacuerdos se exponen en vez de promediarse: iOS tiene `continuous`, que decide
si el valor llega mientras arrastras o solo al soltar; Android tiene `stepSize`,
que convierte el carril en paradas discretas con las marcas que Material les
dibuja.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | — | ✓ | ✓ | ✓ |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UISlider` | `Slider` | `NSSlider` | `Slider` |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `value` | `number \| null` | `0` |  |
| `minimumValue` | `number \| null` | `0` |  |
| `maximumValue` | `number \| null` | `1` |  |
| `color` | `string \| null` | `null` |  |
| `minimumTrackColor` | `string \| null` | `null` | The stretch already covered, from the left to the thumb. |
| `maximumTrackColor` | `string \| null` | `null` | The stretch still to go. |
| `thumbColor` | `string \| null` | `null` |  |

## Eventos

| Evento | Carga | |
|---|---|---|
| `(valueChange)` | `number` |  |

### `[ios]` — lo que tiene UIKit y los demás no

| Clave | Tipo | |
|---|---|---|
| `continuous` | `boolean` | Whether it reports while being dragged or only on release. `UISlider.isContinuous`. Material's always reports while being dragged and that cannot be changed. |

### `[android]` — lo que tiene Android y los demás no

| Clave | Tipo | |
|---|---|---|
| `stepSize` | `number` | The jump between values. `Slider.setStepSize`. |

## Ejemplo

```html
<an-slider
  [value]="volume()"
  [minimumValue]="0"
  [maximumValue]="100"
  [ios]="{ continuous: false }"
  [android]="{ stepSize: 5 }"
  (valueChange)="volume.set($event)" />
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
