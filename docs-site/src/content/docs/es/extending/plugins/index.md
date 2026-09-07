---
title: Plugins
description: Un paquete de npm que además de TypeScript trae código nativo — el contrato, uno escrito de punta a punta, y qué le hace a tu compilación una plataforma que falta.
sidebar:
  order: 1
---

Un plugin es un **paquete de npm que además de TypeScript trae código nativo**.
Cubre lo que el framework no lleva ni tiene por qué llevar: la cámara, la
biometría, las compras dentro de la app, el portapapeles. Lo escribe gente de
fuera de este repositorio, se instala con `npm install`, y `an` hace el resto.

```text
  @tuempresa/plugin-camera
    package.json      angularNative: { module, ios, android }
    src/              la API TypeScript que ve la app
    native/ios/       Swift
    native/android/   Java

  la app
    package.json      "dependencies": { "@tuempresa/plugin-camera": "^1" }
```

Eso es todo lo que declaras. No hay fichero de configuración aparte, ni comando
de instalación, ni nada que registrar a mano: cuando monta el `.app` o el APK,
`an` lee las dependencias de la app, se queda con las que traen manifiesto,
compila sus fuentes junto al shell y genera el registro.

Que esto sea corto no es mérito del diseño, es mérito del terreno: aquí no hay
`.xcodeproj` ni Gradle. Enlazar un plugin en Capacitor o React Native significa
editar el proyecto de Xcode y `settings.gradle` desde un script. Aquí la
compilación ya era «lee unas rutas y pásaselas a `swiftc` y a `javac`», así que
un plugin son unas rutas más.

:::note[Sobre todo métodos, y una vista]
Un plugin es módulos nativos ante todo: JS llama a un método y recibe una
promesa. También puede aportar **un** tipo de cosa al árbol — una vista montada
a través de `<an-custom>` — y ese agujero mide deliberadamente exactamente un
agujero. Mira [Una vista que trae un plugin](#una-vista-que-trae-un-plugin).
:::

## El contrato

Cuatro líneas:

1. **El manifiesto** va en el `package.json` del plugin, bajo `angularNative`, y
   da el nombre del módulo, dónde está la API TypeScript y dónde están las
   fuentes de cada plataforma.
2. **La API TypeScript** es un servicio de Angular normal que llama a
   `NativeModules.call(module, method, args)` y devuelve promesas.
3. **La parte nativa** es un tipo de Swift y una clase de Java que implementan
   `AnPlugin`: reciben un método y unos argumentos, y responden a través de su
   objeto de llamada.
4. **La app lo declara como dependencia** en su `package.json`, y ahí se acaba:
   `an` descubre, compila y registra.

```jsonc
{
  "name": "@angular-native/plugin-clipboard",
  "angularNative": {
    // El nombre con el que lo invoca JS. Único dentro de la app; empieza en
    // minúscula, letras, dígitos y guiones. Este es el único sitio donde se
    // escribe: el registro de Swift y el de Java lo reciben generado.
    "module": "clipboard",

    // El .ts que exporta la API, relativo a la raíz del paquete. Opcional: un
    // plugin publicado en npm ya compilado no lo necesita.
    "entry": "src/public-api.ts",

    // Una sección por plataforma cubierta. La que falta es la que hará fallar
    // la compilación de esa plataforma, a propósito.
    "ios": {
      "sources": "native/ios",           // un directorio, recorrido entero
      "register": "AnClipboardPlugin",   // el tipo de Swift

      // Opcional. Claves de Info.plist y derechos que necesita este plugin.
      "plist": { "NSFaceIDUsageDescription": "…" },
      "entitlements": { "keychain-access-groups": ["…"] }
    },
    "android": {
      "sources": "native/android",
      "register": "dev.angularnative.plugins.ClipboardPlugin",

      // Opcional. Entradas del AndroidManifest.
      "manifest": { "uses-permission": ["…"], "uses-feature": ["…"] }
    }
  }
}
```

Las secciones `plist`, `entitlements` y `manifest` tienen página propia:
[Permisos que necesita un plugin](/es/extending/plugin-permissions/).

## Lo que viene incluido

En este repositorio viven nueve plugins. Se publican como cualquier otro, y son
además los ejemplos trabajados: cada uno es una respuesta real a una pregunta
real de plataforma en lugar de un esbozo, y donde una plataforma no puede hacer
la cosa, la negativa dice por qué.

| Plugin | Qué es | Dónde funciona |
|---|---|:--|
| `plugin-preferences` | `UserDefaults` y `SharedPreferences`, en un almacén propio | iOS · Android · macOS · watchOS |
| `plugin-clipboard` | `UIPasteboard` y `ClipboardManager` | iOS · Android · macOS |
| `plugin-keychain` | El llavero y el almacén de claves de Android | iOS · Android · macOS · watchOS |
| `plugin-biometrics` | Face ID, Touch ID y `BiometricPrompt` | iOS · Android · macOS |
| `plugin-geolocation` | `CLLocationManager` y el `LocationManager` de la plataforma, en primer y segundo plano | iOS · Android · macOS |
| `plugin-camera` | La cámara y la fototeca, presentadas | iOS · Android · macOS |
| `plugin-notifications` | Notificaciones locales, y el toque que lanzó la app | iOS · Android · macOS · watchOS |
| `plugin-updater` | JavaScript nuevo sin publicar en la tienda | iOS · Android · macOS |
| `plugin-barcode` | `AVCaptureMetadataOutput`, a pantalla completa | iOS · macOS |

Cuatro de ellos rechazan una plataforma sin más, y vale la pena leer los motivos
porque son la forma de todos los huecos de este proyecto: un reloj no tiene
cámara ni portapapeles; un escáner en Android necesitaría Play Services o un
descodificador metido en todas las apps; una posición en segundo plano en un
reloj es otra funcionalidad con otro ciclo de vida, no la misma llamada en una
pantalla más pequeña.

`plugin-keychain` y `plugin-biometrics` están explicados juntos en
[Biometría y llavero](/es/extending/biometrics-and-keychain/).

## Escribir uno de punta a punta

Un plugin de haptics — la cosa más pequeña que aún sirve para algo.

### 1. El paquete

```text
packages/plugin-haptics/
  package.json
  src/public-api.ts
  native/ios/AnHapticsPlugin.swift
  native/android/dev/angularnative/plugins/HapticsPlugin.java
```

```jsonc
{
  "name": "@angular-native/plugin-haptics",
  "version": "0.0.1",
  "type": "module",
  "main": "src/public-api.ts",
  "peerDependencies": { "@angular/core": ">=22.0.0" },
  "angularNative": {
    "module": "haptics",
    "entry": "src/public-api.ts",
    "ios": { "sources": "native/ios", "register": "AnHapticsPlugin" },
    "android": {
      "sources": "native/android",
      "register": "dev.angularnative.plugins.HapticsPlugin"
    }
  }
}
```

### 2. La API TypeScript

Un servicio de Angular sin lógica: los tipos de ida y vuelta, y poco más.

```ts
import { inject, Injectable } from '@angular/core'
import { NativeModules } from '@angular-native/platform'

/** Con cuánta fuerza. Los tres que definen ambas plataformas. */
export type HapticStyle = 'light' | 'medium' | 'heavy'

@Injectable({ providedIn: 'root' })
export class Haptics {
  private readonly modules = inject(NativeModules)

  vibrate(style: HapticStyle = 'medium'): Promise<void> {
    return this.modules.call<void>('haptics', 'vibrate', { style })
  }
}
```

El nombre del módulo — `'haptics'` — tiene que coincidir con
`angularNative.module`. Es la única cadena repetida en todo el sistema, y las
dos copias están a dos líneas la una de la otra.

### 3. iOS

El fichero se compila **dentro del `.app`, en la misma invocación de `swiftc`
que el shell**, así que ve `AnPlugin`, `AnPluginCall` y todo UIKit sin importar
nada y sin declarar ninguna dependencia.

```swift
import UIKit

final class AnHapticsPlugin: AnPlugin {
    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall) {
        guard method == "vibrate" else {
            // Nunca en silencio: un método que no existe rechaza.
            respond.reject("the haptics plugin has no method \(method)")
            return
        }
        let style: UIImpactFeedbackGenerator.FeedbackStyle
        switch args["style"] as? String {
        case "light": style = .light
        case "heavy": style = .heavy
        default: style = .medium
        }
        UIImpactFeedbackGenerator(style: style).impactOccurred()
        respond.resolve()
    }
}
```

El protocolo entero:

```swift
protocol AnPlugin: AnyObject {
    /// Opcional. La pantalla de la que cuelga la app, antes de la primera
    /// llamada: de aquí sale un `present` para cualquier cosa que tenga que
    /// mostrar algo.
    func attach(_ host: UIViewController)

    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall)
}
```

Y las respuestas:

```swift
respond.resolve()               // el método no devuelve nada
respond.resolve("algo de texto")
respond.resolve(true)
respond.resolve(3.5)
respond.resolve(["url": path]) // un objeto serializable a JSON
respond.reject("qué salió mal")
```

`args` es lo que haya enviado JS, ya descodificado; si no envió un objeto, llega
vacío.

### 4. Android

Lo mismo con `javac`: se une a la invocación del shell, así que ve `AnPlugin`,
`AnPluginCall` y `android.jar` sin classpath adicional. **La parte de Android de
un plugin es Java, no Kotlin** — mira [Lo que falta](#lo-que-falta).

```java
package dev.angularnative.plugins;

import android.app.Activity;
import android.os.VibrationEffect;
import android.os.Vibrator;

import dev.angularnative.AnPlugin;
import dev.angularnative.AnPluginCall;

import org.json.JSONObject;

public final class HapticsPlugin implements AnPlugin {

    private Activity host;

    @Override
    public void attach(Activity host) {
        this.host = host;
    }

    @Override
    public void call(String method, JSONObject args, AnPluginCall respond) {
        if (!"vibrate".equals(method)) {
            respond.reject("the haptics plugin has no method " + method);
            return;
        }
        Vibrator vibrator = host == null ? null : host.getSystemService(Vibrator.class);
        if (vibrator == null) {
            respond.reject("this device does not vibrate");
            return;
        }
        int amplitude = "heavy".equals(args.optString("style")) ? 255 : 128;
        vibrator.vibrate(VibrationEffect.createOneShot(20, amplitude));
        respond.resolve();
    }
}
```

`attach(Activity)` tiene una implementación por defecto vacía: lo que no
necesite contexto no la escribe. En iOS es `attach(UIViewController)` y funciona
igual.

### 5. Usarlo

En el `package.json` de la app:

```json
"dependencies": { "@angular-native/plugin-haptics": "0.0.1" }
```

En su `tsconfig.json`, la misma ruta que ya toman los paquetes del framework —
`ngc` compila el plugin junto a la app, bajo el mismo `rootDir`:

```jsonc
"paths": {
  "@angular-native/platform": ["../../packages/platform-native/src/public-api.ts"],
  "@angular-native/primitives": ["../../packages/primitives/src/public-api.ts"],
  "@angular-native/plugin-haptics": ["../../packages/plugin-haptics/src/public-api.ts"]
}
```

Y en el componente:

```ts
import { Haptics } from '@angular-native/plugin-haptics'

export class AppComponent {
  private readonly haptics = inject(Haptics)

  confirm(): void {
    this.haptics.vibrate('heavy').catch((error: unknown) => console.error(error))
  }
}
```

Después `npm install`, para que npm enlace el paquete, y:

```bash
an plugins examples/my-app   # ¿se ve?
an ios examples/my-app       # ¿funciona?
```

## Una vista que trae un plugin

Un plugin aporta métodos. Eso fue cierto sin excepción hasta hace poco, y es por
lo que un escáner de códigos de barras solo podía abrirse a pantalla completa:
una previsualización en vivo es una vista, y no había manera de que existiera.

Ahora hay exactamente un agujero, y merece la pena entender por qué tiene esta
forma. `NodeKind` en el core es un enum cerrado cuyos códigos de byte están
congelados en el protocolo — ese byte es la cosa sobre la que tienen que estar
de acuerdo cuatro ficheros en tres lenguajes, y `check-kinds.sh` existe para
mantenerlos de acuerdo. Abrirlo a nombres arbitrarios acabaría con eso. Así que
en su lugar hay **un** tipo nuevo, `Custom`, y el nombre viaja como prop: el
tipo dice «pregunta al registro de vistas de plugin del host», y `an:view` dice
cuál.

Un plugin registra una fábrica:

```swift
AnPluginViews.register("barcode-preview") { AnBarcodePreview() }
```

```java
AnPluginViews.register("barcode-preview", context -> new BarcodePreview(context));
```

y una plantilla la monta como cualquier otra caja:

```html
<an-custom [view]="'barcode-preview'" [style.height]="'260'" [borderRadius]="16" />
```

Del diseño se siguen tres cosas, y las tres sostienen carga:

- **Nunca se mide por su contenido.** Medir significaría que el core llama a un
  plugin durante el layout, en el hilo del motor, y todo el camino de medición
  está construido al revés. Dale un tamaño; una vista sin ninguno sale a cero,
  lo que parece un plugin que no funciona.
- **Un nombre que nadie registró no monta nada** y lo dice una vez en el log,
  nombrando el nombre. Una vez por nombre y no una vez por nodo: una lista de
  quinientas filas con la misma vista ausente es un solo error.
- **En watchOS no.** Ese host refleja el árbol en un modelo que SwiftUI redibuja
  en lugar de montar vistas, así que no hay dónde ponerla.

Esto no es una forma de añadir un primitivo. Un primitivo es un control que el
framework monta en **todos** los hosts, con un nombre sobre el que todas las
capas están de acuerdo y una comprobación que las mantiene de acuerdo. Una vista
de plugin es la vista de una plataforma, montada donde la plantilla lo pidió,
dimensionada por el layout y nada más — y una app que la usa está eligiendo ser
así de menos portable, a propósito.

## Una plataforma que falta se hace notar

**Es la regla más importante de este sistema.** Un plugin que solo cubre iOS,
metido en un APK, no puede acabar siendo un método que devuelve `undefined` y
una pantalla que no reacciona. La compilación se para, antes de compilar nada,
nombrando el plugin y las plataformas que sí cubre, y diciéndote las dos
salidas: el plugin añade su mitad de Android, o la app deja de depender de él.

Puedes preguntar sin compilar:

```bash
an plugins examples/clipboard
# clipboard  (@angular-native/plugin-clipboard)  ios + android  packages/plugin-clipboard

an plugins examples/clipboard --platform android   # sale con 0 si está cubierto, error si no
```

Los cinco hosts cargan plugins. Lo que cambia es lo que un plugin puede *hacer*
en cada uno, y un plugin que no puede funcionar en uno lo declara él mismo: la
compilación rechaza entonces esa combinación y cita la frase del propio plugin,
en lugar de entregar una app cuyas llamadas serían rechazadas en tiempo de
ejecución. Mira
[Plugins en el Mac y en el reloj](/es/extending/plugins-on-the-mac-and-the-watch/).

Y en tiempo de ejecución la misma idea: un método que el plugin no maneja
**rechaza la promesa nombrando el método que se pidió**. Ni se cuelga ni
resuelve con nada. Por eso las dos implementaciones de arriba terminan en un
`reject` en el camino del método desconocido.

Ahora todos los registros hacen esto igual. `call` es `throws` en los shells de
Apple y un error que se escapa se convierte en el rechazo de esa promesa,
nombrando el módulo y el método, exactamente como el registro de Android ha
hecho siempre con una `RuntimeException`. A un plugin que no lance no le afecta:
en Swift un método no lanzador satisface un requisito lanzador, así que nada de
lo que compilaba antes tiene que cambiar — lo que se gana es que un `try` dentro
de un plugin ya no tenga que ser un `try?` que se traga el motivo.

Lo que ningún registro puede cazar es un **trap**: un desempaquetado forzado de
nil, un índice fuera de rango, `fatalError`, un `Error` de Java. Esos se llevan
el proceso por delante y no hay `catch` que llegue.

Así que la única forma que queda de dejar una promesa colgando es que un plugin
se quede su `AnPluginCall` y no llame a ninguno de los dos. Eso es un fallo del
plugin, y todavía no hay ningún plazo que lo corte.

## Cómo funciona por dentro

Tres piezas, y ninguna sabe de las otras más de lo que tiene que saber.

### La cola

El `call` de un módulo nativo se ejecuta en el **hilo del motor de JS**;
`UIPasteboard` y `ClipboardManager` quieren el **hilo de UI**. Así que a un
plugin no se le llama, se le encola:

```text
  hilo del motor                         hilo de UI
  ──────────────────                     ─────────────────────
  HostPlugin::call
       │ encola con su respondedor
       ▼
  PluginBridge ──────── take_calls() ──▶ AnPluginRegistry (Swift/Java)
       ▲                                        │
       └────────── resolve(id, json) ◀──────────┘
```

El hilo de UI recoge las llamadas dentro del fotograma que ya está dibujando — y
responde cuando puede: al momento para el portapapeles, tres segundos después si
hay una cámara detrás. El motor no espera a nadie, y una respuesta que llega
dentro del fotograma se entrega en ese mismo fotograma.

El buzón guarda el respondedor de cada llamada en vuelo, y si se destruye, su
`Drop` las rechaza en lugar de dejarlas esperando para siempre.

Del lado de Rust hay **una sola implementación para todos los plugins**,
`HostPlugin`, y es un cartero. No conoce la lista de métodos de ningún plugin, y
no debería: quien rechaza un método que no existe es la implementación, que es
lo único que conoce su propia lista.

### El registro

`an` genera, en cada compilación, un fichero que enlaza el nombre del manifiesto
con el tipo nativo:

```swift
// build/ios/generated/AnGeneratedPlugins.swift — generado, no editar
enum AnGeneratedPlugins {
    static func install() {
        AnPluginRegistry.register("clipboard", AnClipboardPlugin())
    }
}
```

```java
// build/android/gen-plugins/dev/angularnative/AnGeneratedPlugins.java
public final class AnGeneratedPlugins {
    public static void install() {
        AnPluginRegistry.register("clipboard", new dev.angularnative.plugins.ClipboardPlugin());
    }
}
```

Por esto un plugin **no declara su propio nombre**: declarado en dos sitios —el
manifiesto y el código— podrían dejar de coincidir, y el fallo sería un módulo
que no existe en tiempo de ejecución. Con uno, no puede pasar.

El registro se instala **antes de crear el runtime**: el core construye un módulo
nativo por plugin cuando arranca el motor, y lo que se registrara después no
entraría. Si el shell nunca instalara el despachador, las llamadas rechazan
diciéndolo — y un plugin que la app no lleva rechaza diciendo que no está en
este `.app` ni en este APK.

### El enlazado

El módulo de plugins del CLI hace todo lo demás:

| Paso | Qué hace |
|---|---|
| Descubrir | Lee las `dependencies` del `package.json` de la app, resuelve cada una en `node_modules` como hace Node —subiendo desde el paquete que la pide— y se queda con las que traen `angularNative`. Camina **a través** de los plugins que encuentra, así que un plugin que dependa de otro se lo trae; las dependencias de una librería normal no se siguen, porque nada detrás de una puede ser un plugin que esta app haya decidido llevar. |
| Validar | Un nombre de módulo único y bien formado, un `entry` que existe, directorios de fuentes que existen y no están vacíos. |
| Exigir | Que todos ellos cubran la plataforma que se está compilando. |
| Compilar | Añade sus `.swift` a la línea de `swiftc` y sus `.java` a la de `javac`, junto a los del shell. |
| Registrar | Escribe `AnGeneratedPlugins`. |
| Fundir | Integra sus claves de plist, derechos y entradas de manifiesto en las de la app. |
| Empaquetar | Añade un alias de esbuild por plugin, para que el import del paquete apunte al JS que `ngc` acaba de emitir. |

Ese último alias solo hace falta para plugins que llevan su API TypeScript dentro
de este repositorio. Uno publicado en npm ya viene compilado, y esbuild lo
resuelve a través de `node_modules` como cualquier otra dependencia.

## Comprobarlo sin dispositivo

`scripts/check-plugins.sh`, que se ejecuta dentro de `check-all.sh`, cubre las
tres mitades: que el plugin se descubre, que una plataforma que falta detiene
la compilación, y que una llamada sale y vuelve.

La tercera se prueba con el runner headless, que no tiene ni Swift ni Java pero
tiene todo lo demás. `AN_PLUGINS` monta un módulo de respuestas enlatadas por
nombre:

```bash
an build examples/clipboard
AN_PLUGINS='{"clipboard":{"read":"test text","write":null,"hasText":true}}' \
  cargo run -p an-bridge --example headless -- build/bundle/clipboard/main.js 6
```

```text
-- fake plugin: clipboard
...
Text#15 [20,296 353x19]  "on the clipboard: test text"
```

Sin `AN_PLUGINS` el módulo no existe, y lo que lees en el árbol es el rechazo en
lugar de un hueco silencioso — que es exactamente lo que se está comprobando. Un
método sin respuesta enlatada también se rechaza, con el nombre del método
dentro.

## Lo que falta

- **Una vista de plugin no es un primitivo.** `<an-custom>` monta una, pero
  nunca se mide por su contenido, no existe en watchOS, y no tiene props propias
  más allá de la caja que le da el layout. Un plugin no puede añadir un control
  que el framework monte en todos los hosts —
  [la sección de arriba](#una-vista-que-trae-un-plugin) dice por qué eso sigue
  cerrado.
- **Kotlin.** El shell de Android es Java compilado con `javac` contra
  `android.jar`; aquí no hay Gradle, y sin Gradle no viene ningún `kotlinc` de
  regalo. Un plugin con fuentes `.kt` **detiene la compilación** y lo dice, en lugar
  de producir un APK con esos ficheros callados fuera.
- **Un plazo para el que no responde.** Un plugin que se queda con su objeto de
  llamada y no llama ni a `resolve` ni a `reject` deja la promesa esperando.
  Querría un tiempo límite por llamada —una cámara tarda minutos, leer el
  portapapeles no— y no lo hay.
- **Recursos.** Claves de plist, derechos y entradas de manifiesto un plugin
  *sí* puede aportar. Ficheros propios no.
