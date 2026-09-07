---
title: "@angular-native/plugin-updater"
description: "Publicar JavaScript nuevo sin pasar por la tienda, con un bundle malo costando un arranque — como plugin de angular-native."
sidebar:
  label: updater
  order: 9
---

JavaScript nuevo sin publicar en la tienda. La app es un binario nativo más un
`main.js`: el binario solo puede cambiar por la tienda, el JavaScript es un
fichero, y esto lo reemplaza.

**Un bundle malo cuesta un arranque, no la app.** Un bundle instalado está en
período de prueba: se ejecuta una vez, y si tu código llega a un punto donde está
claramente funcionando y llama a `notifyReady()`, se conserva. Un bundle que
nunca confirma — porque lanzó una excepción, o se colgó — se descarta en el
siguiente arranque y se ejecuta el empaquetado.

`http` se rechaza sin más y se comprueba el `sha256` cuando se da. Este fichero
pasa a ser el código que ejecuta la app; sobre `http`, cualquiera en el camino
elige cuál es ese código.

Ambas tiendas permiten que una app actualice sus propios scripts y ambas prohíben
usar eso para cambiar lo que la app *es*. Correcciones y contenido: para eso
sirve.

```bash
npm install @angular-native/plugin-updater
```

## Dónde funciona

| iOS · iPadOS | macOS | watchOS | Android |
|:--:|:--:|:--:|:--:|
| ✓ | ✓ | — | ✓ |

Donde no funciona, el motivo es del propio plugin y lo dice la compilación, no
la primera llamada en tiempo de ejecución:

**watchOS** — el shell del reloj carga su bundle desde el .app y no tiene ningún sitio escribible donde guardar otro que sobreviva a una actualización de la app de teléfono dentro de la que viaja. El JavaScript de una app de reloj cambia cuando se publica la app en la que está incrustada.

## La API

| Método | Devuelve | |
|---|---|---|
| `current()` | `Promise<InstalledBundle>` |  |
| `notifyReady()` | `Promise<void>` | Dice que este bundle funciona, así que se conserva. |
| `download(request: DownloadRequest)` | `Promise<void>` | Descarga un bundle y lo instala para el siguiente arranque. |
| `reset()` | `Promise<void>` |  |

Cada método es una llamada que cruza el puente, así que todos devuelven una
promesa, y todo fallo es un rechazo que nombra qué se pidió. Mira
[Módulos nativos](/es/reference/native-modules/) para saber qué cruza y qué
aspecto tiene un rechazo.
