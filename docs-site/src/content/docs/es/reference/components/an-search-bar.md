---
title: "an-search-bar"
description: "El campo de búsqueda de la plataforma, con su lupa, su botón de borrar y su forma de cancelar."
sidebar:
  order: 14
---

`UISearchBar`, el `SearchView` de Android, `NSSearchField`. No es un
`an-text-input` con un icono: un campo de búsqueda tiene su propio rol de
accesibilidad, su propia forma de borrarse y, en iOS, su propia relación con la
barra de navegación.

Reporta `(input)` según escribes y `(submit)` con la tecla de retorno, que es la
distinción que importa cuando la búsqueda cuesta una ida y vuelta a la red.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | — | — |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UISearchBar` | `SearchView` | `NSSearchField` | — |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `value` | `string \| null` | `''` |  |
| `placeholder` | `string \| null` | `null` |  |

## Eventos

| Evento | Carga | |
|---|---|---|
| `(input)` | `{ value: string }` |  |
| `(submit)` | `{ value: string }` |  |

## Ejemplo

```html
<an-search-bar
  [value]="query()"
  [placeholder]="'Search'"
  (input)="query.set($event.value)"
  (submit)="run($event.value)" />
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
