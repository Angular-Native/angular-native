# Plugins

Un plugin es un **paquete npm que además de TypeScript trae código nativo**.
Sirve para lo que el framework no lleva dentro y no tiene por qué llevar: la
cámara, la biometría, las compras dentro de la app, el portapapeles. Lo escribe
gente de fuera del repo, se instala con `npm install` y `an` se encarga del
resto.

```text
  @miempresa/plugin-camara
    package.json      angularNative: { module, ios, android }
    src/              la API de TypeScript que ve la app
    native/ios/       Swift
    native/android/   Java

  la app
    package.json      "dependencies": { "@miempresa/plugin-camara": "^1" }
```

Eso es todo lo que hay que declarar. No hay fichero de configuración aparte, ni
un comando de instalación, ni nada que registrar a mano: al armar el `.app` o
el APK, `an` lee las dependencias de la app, se queda con las que traen
manifiesto, compila sus fuentes junto al shell y genera el registro.

Que esto sea corto no es mérito del diseño, es del terreno: aquí no hay
`.xcodeproj` ni Gradle. En Capacitor o React Native, enlazar un plugin es
editar el proyecto de Xcode y el `settings.gradle` desde un script. Aquí el
build ya era «lee unas rutas y pásaselas a `swiftc` y a `javac`», así que un
plugin es unas rutas más.

> **Esta es la fase 1: módulos nativos.** JS llama a un método y recibe una
> promesa. Lo que **no** entra todavía son las *vistas* nativas nuevas —que un
> plugin aporte un `<Camara>` que se monte en el árbol—, porque eso exige abrir
> el `NodeKind` de Rust y tocar los tres hosts. Ver [Lo que falta](#lo-que-falta).

## Índice

- [El contrato](#el-contrato)
- [Escribir uno de principio a fin](#escribir-uno-de-principio-a-fin)
- [Una plataforma que falta da la cara](#una-plataforma-que-falta-da-la-cara)
- [Cómo funciona por dentro](#cómo-funciona-por-dentro)
- [Comprobarlo sin dispositivo](#comprobarlo-sin-dispositivo)
- [Lo que falta](#lo-que-falta)

## El contrato

Cuatro líneas:

1. **El manifiesto** va en el `package.json` del plugin, bajo `angularNative`, y
   dice el nombre del módulo, dónde está la API de TypeScript y dónde las
   fuentes de cada plataforma.
2. **La API de TypeScript** es un servicio de Angular normal que llama a
   `NativeModules.call(módulo, método, args)` y devuelve promesas.
3. **Lo nativo** es un tipo Swift y una clase Java que implementan `AnPlugin`:
   reciben método y argumentos, y contestan por su `AnPluginCall`.
4. **La app lo declara como dependencia** en su `package.json`, y ahí se acaba:
   `an` descubre, compila y registra.

El manifiesto completo:

```jsonc
{
  "name": "@angular-native/plugin-clipboard",
  "angularNative": {
    // El nombre con el que JS lo invoca. Único en la app; minúscula inicial,
    // letras, cifras y guiones. Es el único sitio donde se escribe: el
    // registro de Swift y el de Java lo reciben generado.
    "module": "clipboard",

    // El .ts que exporta la API, relativo a la raíz del paquete. Opcional: un
    // plugin publicado en npm que ya venga compilado no lo necesita.
    "entry": "src/public-api.ts",

    // Una sección por plataforma cubierta. La que no esté es la que hará
    // fallar el build de esa plataforma, a propósito.
    "ios": {
      "sources": "native/ios",              // directorio, se recorre entero
      "register": "AnClipboardPlugin"       // el tipo Swift
    },
    "android": {
      "sources": "native/android",
      "register": "dev.angularnative.plugins.ClipboardPlugin"  // con su paquete
    }
  }
}
```

El plugin de referencia está en
[`packages/plugin-clipboard`](../packages/plugin-clipboard), y la app que lo usa
en [`examples/clipboard`](../examples/clipboard). Son ~200 líneas entre todo.

## Escribir uno de principio a fin

Se escribe uno de vibración, que es lo más pequeño que sigue siendo útil.

### 1. El paquete

```text
packages/plugin-haptics/
  package.json
  src/public-api.ts
  native/ios/AnHapticsPlugin.swift
  native/android/dev/angularnative/plugins/HapticsPlugin.java
```

```jsonc
// packages/plugin-haptics/package.json
{
  "name": "@angular-native/plugin-haptics",
  "version": "0.0.1",
  "private": true,
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

### 2. La API de TypeScript

Un servicio de Angular sin lógica: los tipos de ida y vuelta y poco más.

```ts
// packages/plugin-haptics/src/public-api.ts
import { inject, Injectable } from '@angular/core'
import { NativeModules } from '@angular-native/platform'

/** Fuerza del golpe. Las tres que definen las dos plataformas. */
export type HapticStyle = 'light' | 'medium' | 'heavy'

@Injectable({ providedIn: 'root' })
export class Haptics {
  private readonly modules = inject(NativeModules)

  vibrate(style: HapticStyle = 'medium'): Promise<void> {
    return this.modules.call<void>('haptics', 'vibrate', { style })
  }
}
```

El nombre del módulo —`'haptics'`— tiene que ser el mismo que el
`angularNative.module`. Es la única cadena que se repite en todo el sistema, y
está a dos líneas de distancia.

### 3. iOS

El fichero se compila **dentro del `.app`, en la misma invocación de `swiftc`
que el shell**, así que ve `AnPlugin`, `AnPluginCall` y todo UIKit sin importar
nada ni declarar dependencias.

```swift
// packages/plugin-haptics/native/ios/AnHapticsPlugin.swift
import UIKit

final class AnHapticsPlugin: AnPlugin {
    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall) {
        guard method == "vibrate" else {
            // Nunca en silencio: un método que no existe rechaza la promesa.
            respond.reject("el plugin haptics no tiene ningún método \(method)")
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
    /// llamada: de aquí sale el `present` de quien tenga que enseñar algo.
    func attach(_ host: UIViewController)

    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall)
}
```

Y las respuestas:

```swift
respond.resolve()               // el método no devuelve nada
respond.resolve("un texto")
respond.resolve(true)
respond.resolve(3.5)
respond.resolve(["url": ruta])  // un objeto serializable a JSON
respond.reject("lo que salió mal")
```

`args` es lo que mandó JS ya decodificado; si no mandó un objeto llega vacío.

### 4. Android

Lo mismo con `javac`: entra en la misma invocación que el shell, así que ve
`AnPlugin`, `AnPluginCall` y `android.jar` sin classpath adicional. **El lado
Android de un plugin es Java, no Kotlin** — ver [Lo que falta](#lo-que-falta).

```java
// packages/plugin-haptics/native/android/dev/angularnative/plugins/HapticsPlugin.java
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
            respond.reject("el plugin haptics no tiene ningún método " + method);
            return;
        }
        Vibrator vibrator = host == null ? null : host.getSystemService(Vibrator.class);
        if (vibrator == null) {
            respond.reject("este aparato no vibra");
            return;
        }
        int amplitude = "heavy".equals(args.optString("style")) ? 255 : 128;
        vibrator.vibrate(VibrationEffect.createOneShot(20, amplitude));
        respond.resolve();
    }
}
```

`attach(Activity)` tiene implementación por defecto vacía: quien no necesite un
contexto no la escribe. En iOS es `attach(UIViewController)` y funciona igual.

### 5. Usarlo

En el `package.json` de la app:

```json
"dependencies": { "@angular-native/plugin-haptics": "0.0.1" }
```

En su `tsconfig.json`, la misma ruta que ya llevan los paquetes del framework
—`ngc` compila el plugin junto a la app, bajo el mismo `rootDir`—:

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

  confirmar(): void {
    this.haptics.vibrate('heavy').catch((error: unknown) => console.error(error))
  }
}
```

`npm install` para que npm enlace el paquete, y ya:

```bash
cargo an plugins examples/mi-app     # a ver si lo ve
cargo an ios examples/mi-app         # a ver si funciona
```

## Una plataforma que falta da la cara

**Es la regla más importante de este sistema.** Un plugin que solo cubre iOS
metido en un APK no puede acabar en un método que devuelve `undefined` y una
pantalla que no reacciona. Se para el build:

```console
$ cargo an android examples/mi-app
Error: esta app no se puede compilar para Android: 1 de sus plugins no lo cubre
  · @miempresa/plugin-camara (módulo "camara") solo trae ios

O el plugin añade su parte de Android —fuentes en angularNative.android de su
package.json—, o la app deja de depender de él.
```

Lo mismo al revés, y lo mismo con el reloj, que todavía no carga plugins:

```console
$ cargo an watchos examples/mi-app
Error: esta app no se puede compilar para watchOS: el reloj todavía no carga
plugins, y depende de @angular-native/plugin-clipboard. Ver docs/plugins.md.
```

Se puede preguntar sin llegar a compilar:

```bash
cargo an plugins examples/clipboard
# clipboard  (@angular-native/plugin-clipboard)  ios + android  packages/plugin-clipboard

cargo an plugins examples/clipboard --platform android   # 0 si lo cubre, error si no
```

Y en tiempo de ejecución la misma idea: un método que el plugin no atiende
**rechaza la promesa diciendo cuál se pidió**, no la deja colgada ni la resuelve
con nada. Por eso las dos implementaciones de arriba acaban en un `reject` en el
camino del método desconocido, y por eso el registro convierte en rechazo
cualquier excepción que se escape del plugin.

La única forma de dejar una promesa colgada es que el plugin **no conteste**:
que se guarde el `AnPluginCall` y no llame ni a `resolve` ni a `reject`. Eso es
un bug del plugin, y de momento no hay plazo que lo corte —está en [Lo que
falta](#lo-que-falta)—. Todo camino de error tiene que acabar en `reject`.

## Cómo funciona por dentro

Tres piezas, y ninguna sabe de las otras más de lo justo.

### La cola

`NativeModule::call` corre en el **hilo del motor JS**; `UIPasteboard` y
`ClipboardManager` quieren el **hilo de UI**. Así que un plugin no se llama, se
encola:

```text
  hilo del motor                         hilo de UI
  ──────────────────                     ─────────────────────
  HostPlugin::call
       │ encola con su Responder
       ▼
  PluginBridge ──────── take_calls() ──▶ AnPluginRegistry (Swift/Java)
       ▲                                        │
       └────────── resolve(id, json) ◀──────────┘
```

El hilo de UI recoge las llamadas dentro del frame que ya está haciendo —
`an_runtime_frame` en iOS, `nativeFrame` en Android— y contesta cuando puede: en
el acto para el portapapeles, tres segundos después si detrás hay una cámara. El
motor no espera a nadie, y una respuesta que llega dentro del frame se entrega en
ese mismo frame.

El buzón es [`crates/an-bridge/src/plugins.rs`](../crates/an-bridge/src/plugins.rs).
Guarda el `Responder` de cada llamada en vuelo, y si el buzón se destruye su
`Drop` las rechaza en vez de dejarlas esperando para siempre.

Del lado de Rust hay **una sola implementación** para todos los plugins,
`HostPlugin`, que hace de cartero. No sabe qué métodos tiene ninguno, y no le
corresponde: quien rechaza un método que no existe es la implementación, que es
la única que conoce su propia lista.

### El registro

`an` genera, en cada build, un fichero que enlaza el nombre del manifiesto con
el tipo nativo:

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

Por eso el plugin **no declara su propio nombre**: si lo declarase en dos sitios
—el manifiesto y el código— podrían dejar de coincidir, y el fallo sería un
módulo que no existe en tiempo de ejecución. Con uno solo, no puede pasar.

El registro se instala **antes de crear el runtime**: el core construye un
módulo nativo por plugin al arrancar el motor, y lo que se registre después no
entraría.

### El enlazado

[`crates/an-cli/src/plugins.rs`](../crates/an-cli/src/plugins.rs), unas 350
líneas, hace todo lo demás:

| Paso | Qué hace |
|---|---|
| Descubrir | Lee las `dependencies` del `package.json` de la app, resuelve cada una en `node_modules` —primero el de la app, luego el de la raíz, como Node— y se queda con las que traen `angularNative` |
| Validar | Nombre de módulo único y bien formado, `entry` que existe, directorios de fuentes que existen y no están vacíos |
| Exigir | Que todas cubran la plataforma que se está compilando |
| Compilar | Añade sus `.swift` a la línea de `swiftc` y sus `.java` a la de `javac`, junto a las del shell |
| Registrar | Escribe `AnGeneratedPlugins` |
| Empaquetar | Añade un `--alias` de esbuild por plugin, para que el import del paquete apunte al JS que acaba de salir de `ngc` |

Ese último alias solo hace falta para los plugins que traen su API en
TypeScript dentro del repo. Uno publicado en npm ya viene compilado, y entonces
esbuild lo resuelve por `node_modules` como cualquier otra dependencia.

## Comprobarlo sin dispositivo

[`scripts/check-plugins.sh`](../scripts/check-plugins.sh), que corre dentro de
`check-all.sh`, cubre las tres mitades: que el plugin se descubra, que la
plataforma que falta pare el build, y que la llamada vaya y vuelva.

Lo tercero se prueba con el runner headless, que no tiene ni Swift ni Java pero
sí todo lo demás. `AN_PLUGINS` le monta un módulo de respuestas fijas por cada
nombre:

```bash
cargo an build examples/clipboard
AN_PLUGINS='{"clipboard":{"read":"texto de prueba","write":null,"hasText":true}}' \
  cargo run -p an-bridge --example headless -- build/bundle/clipboard/main.js 6
```

```text
-- plugin de mentira: clipboard
...
Text#15 [20,296 353x19]  "en el portapapeles: texto de prueba"
```

Sin `AN_PLUGINS` el módulo no existe, y en el árbol se lee el rechazo en vez de
un hueco silencioso — que es exactamente lo que se quiere comprobar:

```text
Text#17 [20,331 353x32]  "el portapapeles falló: Error: no hay nin…"
```

Un método sin respuesta preparada se rechaza igual, con el nombre del método
dentro.

## Lo que falta

- **Vistas nativas.** Un plugin puede aportar métodos, no primitivas. Que traiga
  un `<Camara>` que se monte en el árbol exige abrir el `NodeKind` de
  `an-core` a nombres que el core no conoce en tiempo de compilación, y que los
  tres hosts sepan construir una vista que no es suya. Es la fase 2, y es la
  parte gorda.
- **Kotlin.** El shell de Android es Java y se compila con `javac` contra
  `android.jar`; aquí no hay Gradle, y sin Gradle no hay `kotlinc` que valga de
  serie. Un plugin con fuentes `.kt` **detiene el build** y lo dice, en vez
  de compilar el APK sin esos ficheros dentro.
- **El reloj.** `an-watch` no tiene el registro que tienen `an-ios` y
  `an-android`, y su shell no es un port del de iOS. Una app con plugins no
  compila para watchOS, y lo dice.
- **Plazo para el que no contesta.** Un plugin que se guarda el `AnPluginCall` y
  no llama ni a `resolve` ni a `reject` deja la promesa esperando. Habría que
  cortarla con un plazo configurable por llamada —una cámara tarda minutos, leer
  el portapapeles no—, y de momento no lo hay.
- **Dependencias de dependencias.** Se miran las `dependencies` directas de la
  app. Un plugin que a su vez dependa de otro plugin no arrastra al segundo.
- **Recursos y permisos.** Un plugin no puede aportar todavía entradas al
  `Info.plist` ni al `AndroidManifest.xml`, ni assets propios. La cámara los
  necesita, así que esto va justo detrás de las vistas.
