---
title: "an-scroll-view"
description: "Un contenedor con scroll, con tirar-para-recargar y desplazamiento — y los dos estilos que necesita de su padre o no hará scroll en absoluto."
sidebar:
  order: 4
---

La vista con scroll del sistema: `UIScrollView`, el `ScrollView` de Android,
`NSScrollView`. La inercia, el rebote en los bordes, el comportamiento de la
barra y cómo interactúa con el teclado son de la plataforma, no una imitación.

El núcleo reporta `contentSize` —cuánto ocupan los hijos— calculado asumiendo
que el desbordamiento va **hacia abajo**. Así que el scroll horizontal no es una
prop de host que falte: es trabajo en `an-core`.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UIScrollView` | `ScrollView` | `NSScrollView` | `ScrollView` |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `showsScrollIndicator` | `boolean \| null` | `null` |  |
| `scrollEnabled` | `boolean \| null` | `true` | Whether the finger moves the content. |
| `bounces` | `boolean \| null` | `null` | iOS's bounce on reaching the end. |
| `refreshing` | `boolean \| null` | `false` | Whether it is refreshing. Setting it to `false` closes the spinner; the gesture itself opens it, not this prop. |

## Eventos

| Evento | Carga | |
|---|---|---|
| `(refresh)` | — | Pull to refresh. |
| `(scroll)` | `{ x, y }` | Emitted on every frame of the scroll. Layout computes the `contentSize` by itself: it is the size the children take up, and the core sends it to the `UIScrollView` whenever it changes. |

### `[ios]` — lo que tiene UIKit y los demás no

| Clave | Tipo | |
|---|---|---|
| `pagingEnabled` | `boolean` | Scrolling comes to rest at multiples of the view's size. `UIScrollView.isPagingEnabled`. Android does not ship it: its answer is `ViewPager2`, which is another view with its own adapter, not a prop. |
| `keyboardDismissMode` | `'none' | 'onDrag' | 'interactive'` | What the keyboard does while scrolling. `keyboardDismissMode`. On Android the keyboard does not hide on scroll and there is nothing to ask it for. |

### Hay que dejarle ser más pequeño que su contenido

Una vista con scroll medida a la altura de su contenido no es una vista con
scroll: es una columna muy alta, y quien hace scroll es la página. Necesita una
parte del espacio y permiso para encoger:

```html
<an-view [style.flexGrow]="'1'">
  <an-scroll-view [style.flexGrow]="'1'" [style.minHeight]="'0'" [style.overflow]="'scroll'">
    …
  </an-scroll-view>
</an-view>
```

Esta es, de largo, la razón más común de que una vista con scroll «no haga
nada».

## Ejemplo

```html
<an-scroll-view
  [style.flexGrow]="'1'"
  [style.overflow]="'scroll'"
  [refreshing]="loading()"
  (refresh)="reload()"
  (scroll)="offset.set($event.y)">
  @for (row of rows(); track row.id) {
    <an-text>{{ row.name }}</an-text>
  }
</an-scroll-view>
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
