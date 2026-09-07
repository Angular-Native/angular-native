---
title: "an-tab-bar"
description: "La barra de navegación inferior — un UITabBarController de verdad, un BottomNavigationView, y en el Mac el control que usan de verdad las apps de Mac."
sidebar:
  order: 15
---

`UITabBarController` en iOS, `BottomNavigationView` en Android. En el Mac es un
`NSSegmentedControl`, porque una app de Mac no tiene barra de pestañas abajo y
poner una allí sería una app de móvil dentro de una ventana.

Es un control, no un outlet del router. Reporta `(select)` con un índice y tú
decides qué significa: cambiar una señal, o navegar. No lleva una pila de rutas
por pestaña como hace `UITabBarController` de forma nativa.

`[icons]` admite los mismos nombres que `an-icon`.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | — | — |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UITabBarController` | `BottomNavigationView` | `NSSegmentedControl` | — |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `items` | `readonly string[] \| null` | `null` |  |
| `icons` | `readonly string[] \| null` | `null` | Icons, in the same order as the titles. |
| `selectedIndex` | `number \| null` | `0` |  |
| `color` | `string \| null` | `null` | The active tab's colour. |
| `unselectedColor` | `string \| null` | `null` | The colour of the rest. |

## Eventos

| Evento | Carga | |
|---|---|---|
| `(select)` | `number` |  |

### `[ios]` — lo que tiene UIKit y los demás no

| Clave | Tipo | |
|---|---|---|
| `translucent` | `boolean` | Whether what goes past behind it shows through. `UITabBar.isTranslucent`. Material's bar is opaque by design and has no switch for this. |

## Ejemplo

```html
<an-tab-bar
  [items]="['Home', 'Search', 'Profile']"
  [icons]="['home', 'search', 'profile']"
  [selectedIndex]="tab()"
  (select)="tab.set($event)" />
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
