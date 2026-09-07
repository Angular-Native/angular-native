---
title: "@angular-native/plugin-clipboard"
description: "El portapapeles del sistema, como plugin de angular-native."
sidebar:
  label: clipboard
  order: 3
---

El portapapeles del sistema: `UIPasteboard` en iOS, `ClipboardManager` en
Android, `NSPasteboard` en el Mac. Es además el plugin de referencia — el
ejemplo completo más pequeño del contrato, explicado de punta a punta en la
[página de plugins](/es/extending/plugins/).

```bash
npm install @angular-native/plugin-clipboard
```

## Dónde funciona

| iOS · iPadOS | macOS | watchOS | Android |
|:--:|:--:|:--:|:--:|
| ✓ | ✓ | — | ✓ |

Donde no funciona, el motivo es del propio plugin y lo dice la compilación, no
la primera llamada en tiempo de ejecución:

**watchOS** — watchOS no tiene portapapeles del sistema: UIPasteboard es API_UNAVAILABLE(watchos) y no hay nada que ocupe su lugar. Un reloj comparte texto entregándoselo al teléfono emparejado, que es otra funcionalidad con otra API, no un portapapeles.

## La API

| Método | Devuelve | |
|---|---|---|
| `write(text: string)` | `Promise<void>` |  |
| `read()` | `Promise<string>` | Lo que se haya copiado, o una cadena vacía si no hay texto. |
| `hasText()` | `Promise<boolean>` |  |

Cada método es una llamada que cruza el puente, así que todos devuelven una
promesa, y todo fallo es un rechazo que nombra qué se pidió. Mira
[Módulos nativos](/es/reference/native-modules/) para saber qué cruza y qué
aspecto tiene un rechazo.
