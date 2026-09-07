---
title: Módulos integrados
description: Ficheros, compartir, estado de la red y háptico — en qué se convierte cada uno en cada una de las ocho plataformas, y qué dicen las que no pueden tenerlo.
sidebar:
  order: 5
---

Un [módulo nativo](/es/reference/native-modules/) es un método que JS puede
llamar. Un [plugin](/es/extending/plugins/) es uno que llega como paquete de npm.
Un **integrado** es uno que trae el framework: nadie lo declara, no hay
dependencia que añadir, y lo tienen todos los hosts.

Hay cinco. [`Device`](/es/reference/native-modules/) contesta desde el hilo del
motor porque lo que dice no cambia nunca. Los otros cuatro no pueden: una hoja de
compartir, un selector de ficheros, un vibrador y un monitor de red viven todos
en el hilo de UI, así que salen por una cola y contestan cuando la plataforma
llega a ellos — que es el mismo camino que toma la llamada de un plugin, con un
buzón propio para que nada instalado desde npm pueda ponerse delante de un nombre
que promete el framework.

```ts
import { inject } from '@angular/core'
import { Files, Haptics, Network, Share } from '@angular-native/platform'

export class Draft {
  private readonly files = inject(Files)
  private readonly sharing = inject(Share)
  private readonly haptics = inject(Haptics)

  async save(text: string): Promise<void> {
    await this.files.write('draft.txt', text)
    await this.haptics.notification('success')
  }

  async send(): Promise<void> {
    const home = await this.files.documentsDirectory()
    await this.sharing.share({ title: 'Borrador', files: [`${home}/draft.txt`] })
  }
}
```

## Qué es cada uno, por plataforma

| | iOS · iPadOS | tvOS | visionOS | macOS | watchOS | Android | Wear OS |
|---|---|---|---|---|---|---|---|
| **files** — contenedor | `Documents` | *solo caché* | `Documents` | `Application Support` | `Documents` | `getFilesDir()` | `getFilesDir()` |
| **files** — selector | `UIDocumentPickerViewController` | **ninguno** | `UIDocumentPickerViewController` | `NSOpenPanel` | **ninguno** | `ACTION_OPEN_DOCUMENT` | **ninguno** |
| **share** | `UIActivityViewController` | **ninguno** | `UIActivityViewController` | `NSSharingServicePicker` | **ninguno** | selector `ACTION_SEND` | selector `ACTION_SEND`¹ |
| **network** | `NWPathMonitor` | `NWPathMonitor` | `NWPathMonitor` | `NWPathMonitor` | `NWPathMonitor` | `NetworkCallback` | `NetworkCallback` |
| **haptics** | `UIImpactFeedbackGenerator` | **ninguno** | **ninguno** | `NSHapticFeedbackManager`² | `WKInterfaceDevice.play` | `VibrationEffect` | `VibrationEffect` |

¹ Un reloj Wear resuelve `ACTION_SEND` a Bluetooth y a lo que haya añadido el
fabricante, así que el selector es real pero puede tener una entrada o ninguna.
Pregunta con `canShare()`: en esta plataforma contesta el reloj, no la
plataforma.

² Solo si este Mac tiene un trackpad Force Touch, y AppKit no ofrece forma de
preguntar si lo tiene. Ver `support().caveat`.

## Qué dice una plataforma que no lo tiene

No se emula nada y nada se queda callado sin hacer nada. Donde no está el
hardware o no está la clase, la promesa se rechaza con una frase que nombra la
plataforma, dice qué falta y dice qué hacer en su lugar:

- **`files.documentsDirectory()` en tvOS** — una tele no le da a una app
  almacenamiento que dure; el sistema puede vaciar el contenedor en cualquier
  momento. Apunta a `cacheDirectory()`, que es donde aterriza allí una ruta
  relativa.
- **`files.pick()` en tvOS, watchOS y Wear OS** — ninguna de las tres trae un
  explorador de documentos, y en Wear `ACTION_OPEN_DOCUMENT` no resuelve a nada.
- **`share.share()` en tvOS y watchOS** — no hay `UIActivityViewController`, no
  hay AirDrop, no hay a quién darle nada.
- **`haptics.*` en tvOS y visionOS** — una tele no tiene nada que vibrar y el
  Siri Remote no tiene un motor que una app pueda mover; en el visor no se
  sostiene nada y nada toca la muñeca.
- **`haptics.notification()` en macOS** — `NSHapticFeedbackManager` tiene tres
  patrones y ninguno significa éxito, aviso o error, así que tocar uno para los
  tres serían tres significados saliendo como una sola sensación.

Todas estas están además en los tipos, para que una app se pueda escribir contra
ellas en vez de enterarse capturando:

```ts
import type {
  FilePickerPlatform, // todas las plataformas menos tvos, watchos y wearos
  HapticsPlatform,    // todas menos tvos y visionos
  NetworkPlatform,    // todas: esta no tiene excepciones
  SharePlatform       // todas menos tvos y watchos
} from '@angular-native/platform'
```

Y donde la respuesta depende del dispositivo y no de la plataforma, hay un método
para preguntar: `share.canShare()` y `haptics.support()`.

## Dónde pueden ir los ficheros

`files` traza una raya y la hace cumplir en vez de documentarla:

- **Escribe** solo dentro de `documentsDirectory()` y `cacheDirectory()`.
- Fuera de esos dos solo **lee** lo que haya devuelto `pick()`, y solo mientras la
  app esté corriendo.

Cualquier otra cosa se rechaza diciendo cuál de las dos ha incumplido. Sin esa
regla, una ruta que llega desde JS es una ruta a cualquier sitio del disco. Una
ruta relativa se resuelve contra `documentsDirectory()`, así que
`read('notas.txt')` significa algo sin que la app tenga que preguntar dónde está
eso.

En Android, lo que devuelve `pick()` es una URI `content://` y no una ruta: es lo
que entrega el Storage Access Framework, el documento puede no estar siquiera en
el dispositivo, y una ruta con `/` inventada para él sería una ruta a nada. Pásala
tal cual de vuelta a `read()` o a `share()`.

## Permisos

Dos de los cuatro necesitan algo en el manifiesto de Android, y los dos son de
**tiempo de instalación**: no hay diálogo ni nada que la persona conceda después,
así que una app a cuyo manifiesto le falte la línea tiene un módulo que solo
puede fallar.

| Módulo | Permiso | Sin él |
|---|---|---|
| `network` | `android.permission.ACCESS_NETWORK_STATE` | `status()` rechaza con la línea que hay que pegar |
| `haptics` | `android.permission.VIBRATE` | todos los métodos rechazan con la línea que hay que pegar |

El propio shell del framework declara los dos, así que una app construida con
`an android` los tiene. Cada módulo comprueba además antes de llamar, que es la
diferencia entre que te digan qué falta y morirse con una `SecurityException` al
primer toque.

Compartir un fichero necesita una cosa más, y no es un permiso: desde Android 7
una ruta `file://` no puede cruzar a otra app. El shell declara un
`FileProvider` sobre los dos directorios que posee `files`, y `share` envuelve lo
que manda. Una app cuyo manifiesto no tenga ese proveedor recibe el bloque
`<provider>` para pegar en vez de un cierre inesperado.

Nada en las plataformas de Apple necesita una cadena de uso para estos cuatro.
Una app de Mac en sandbox que quiera `files.pick()` necesita
`com.apple.security.files.user-selected.read-only` en sus entitlements; el shell
de aquí no está en sandbox, así que no lo lleva.

## La señal de red

`Network` es el que tiene algo que vigilar:

```ts
const network = inject(Network)
network.watch()               // mantiene fresco `network.status()`
network.status()              // Signal<NetworkStatus | null>
```

El monitor de debajo funciona por eventos: es el sistema empujando en el momento
en que se cae el Wi-Fi.

Esto antes hacía *polling*. No había forma de que un módulo nativo alcanzara JS:
el puente llevaba respuestas a llamadas y nada más, así que `watch()` preguntaba
cada cierto tiempo y este párrafo lo decía. Ese canal ya existe —ver
[eventos de módulo](/es/reference/native-modules/#eventos-lo-que-un-módulo-dice-sin-que-le-pregunten)—
y `watch()` conserva la forma que siempre tuvo y ha perdido el temporizador.

## Probarlos

`examples/modules` llama a los cuatro e imprime una línea por respuesta, los
rechazos incluidos:

```bash
cargo an macos examples/modules
cargo an android examples/modules
cargo an tvos examples/modules      # para ver qué rechaza una tele
cargo an watchos examples/modules   # y qué hace un reloj
```

`scripts/check-builtins.sh` lee esas líneas de la app de macOS en marcha, y
`scripts/check-builtins-device.sh` hace lo mismo por `adb` sobre un móvil o un
reloj — que es la única forma de averiguar qué puede recibir de verdad un
dispositivo Wear concreto.
