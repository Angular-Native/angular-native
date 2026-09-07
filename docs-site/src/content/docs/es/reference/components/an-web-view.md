---
title: "an-web-view"
description: "Un WKWebView o un android.webkit.WebView de verdad, para la página que de verdad necesitas — y las dos plataformas que no tienen uno."
sidebar:
  order: 23
---

`WKWebView` en iOS y en el Mac, `android.webkit.WebView` en Android. Admite o una
`[url]` o una cadena de `[html]`.

No hay ironía en que un framework que existe para evitar WebViews traiga uno: una
página de condiciones, un flujo de OAuth o un artículo con formato son un
documento, y un documento es para lo que sirve una vista web. Para lo que no
sirve es para la interfaz.

**En tvOS no**, donde WebKit no está en el SDK — y esa ausencia es un `cfg` que
mantiene el framework fuera del binario, no un stub que falla en ejecución. En
watchOS tampoco, por lo mismo.

## Disponibilidad

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | — | ✓ | — | — |

## En qué se convierte

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `WKWebView` | `android.webkit.WebView` | `WKWebView` | — |

## Props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `url` | `string \| null` | `null` |  |
| `html` | `string \| null` | `null` |  |

## Ejemplo

```html
<an-web-view [url]="'https://angular-native.github.io/'" [style.flexGrow]="'1'" />
```

Todas las primitivas llevan además las props de la base —fondo, borde, opacidad, las transformaciones, `animate`, el contrato de accesibilidad— y todos los gestos. La base completa está en [Componentes](/es/reference/components/), y las cargas en [Eventos](/es/reference/events/).
