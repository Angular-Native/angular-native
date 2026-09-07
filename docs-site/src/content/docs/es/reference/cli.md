---
title: El CLI `an`
description: Todos los comandos, todas las banderas y todos los valores por defecto, leídos del código — y el puñado de asimetrías entre ellos que conviene conocer antes de topártelas.
sidebar:
  label: El CLI an
  order: 3
---

`an` es toda la cadena de herramientas. No hay `.xcodeproj`, no hay Gradle y no
hay proyecto generado que mantener sincronizado: mueve él mismo `ngc`, esbuild,
`cargo`, `swiftc`, `aapt2`, `javac`, `d8` y `apksigner`, y el único fichero que
deja en tu repositorio es un manifiesto pequeño.

```bash
cargo install --path crates/an-cli
```

Hay **doce comandos** y ningún subcomando anidado. Todas las opciones son de
forma larga: en todo el CLI no hay ni una bandera corta.

| Comando | Qué hace |
|---|---|
| `an init [DIR]` | Prepara un proyecto de Angular. |
| `an add <PLATAFORMA>` | Crea el directorio de esa plataforma en el proyecto. |
| `an build [APP]` | Compila y empaqueta el JS, y ahí se para. |
| `an ios [APP]` | Compila y ejecuta en el simulador de iOS. |
| `an tvos [APP]` | El simulador de Apple TV. |
| `an visionos [APP]` | El simulador de Vision Pro. |
| `an watchos [APP]` | El simulador de Apple Watch. |
| `an macos [APP]` | Construye un `.app` y lo abre en esta máquina. |
| `an android [APP]` | Construye un APK y lo instala en un móvil conectado. |
| `an wearos [APP]` | Lo mismo, para un dispositivo con forma de reloj. |
| `an plugins [APP]` | Lista los plugins que arrastra una app, y opcionalmente comprueba una plataforma. |
| `an dev [APP]` | Sirve el bundle, vigila, y recarga en caliente la app en marcha. |

## `an init`

```bash
an init [DIR] [--name <NOMBRE>] [--id <ID>] [--force]
```

| Argumento | Por defecto |
|---|---|
| `DIR` | el directorio actual |
| `--name` | el `app.name` anterior, si no el PascalCase del `name` de `package.json` |
| `--id` | el `app.bundleId` anterior, si no `dev.angularnative.<slug>` |
| `--force` | apagado — reescribe solo lo que genera `an`, nunca tu código |

Se niega a ejecutarse salvo que haya un `angular.json` **y** `@angular/core` en
las dependencias. El identificador de bundle tiene que tener al menos dos
segmentos separados por puntos, cada uno empezando por una letra ASCII y con solo
alfanuméricos ASCII: ni guiones ni guiones bajos.

Lo que deja detrás:

| Ruta | Qué es |
|---|---|
| `angular-native.json` | El manifiesto: nombre, id de bundle, entrada, plataformas. Se reescribe siempre. |
| `.angular-native/tsconfig.json` | El tsconfig de la compilación nativa — AOT completo, plantillas estrictas, `types: []`. `--force` lo reescribe. |
| `src/main.native.ts` | El punto de entrada nativo. **Nunca** se reescribe, con `--force` o sin él. |
| `src/app/app-native.ts` | Un componente raíz del que partir. Tampoco se reescribe nunca. |
| `.angular-native/vendor/*.tgz` | Los dos paquetes del framework, compilados con *tu* `ngc` en modo parcial y empaquetados. Están pensados para commitearse. |
| `.gitignore` | Una línea añadida, `/.angular-native/build/`, y solo si no estaba ya. |
| `package.json` | Los dos paquetes vendorizados instalados por ruta; `@angular/compiler-cli` añadido como dependencia de desarrollo si no hay `ngc` alcanzable. |

Todo lo que puede fallar corre antes de que se escriba el manifiesto, así que un
`init` fallido deja el proyecto sin inicializar en vez de a medio inicializar. Lo
que ya exista se deja en paz y se reporta como tal.

**No toca ni `angular.json`, ni `src/main.ts`, ni el `tsconfig.json` de la web.**
`ng build` y `ng serve` siguen funcionando exactamente igual que antes.

## `an add`

```bash
an add <PLATAFORMA>
```

La plataforma es obligatoria, y hay exactamente cinco: **`ios`, `tvos`,
`visionos`, `macos`, `android`**. No hay `an add watchos` ni `an add wearos`.

Cada uno crea un directorio con exactamente un fichero dentro —`ios/Info.plist`,
`tvos/Info.plist`, `visionos/Info.plist`, `macos/Info.plist`,
`android/AndroidManifest.xml`— derivado del del shell, con el nombre y el id de
bundle sustituidos. Si el fichero ya está, se deja en paz; en cualquier caso la
plataforma queda anotada en `angular-native.json`.

`macos` decora los dos igual que `tvos` y `visionos`: una app llamada `MyApp`
pasa a ser `MyAppMac` con `.mac` al final de su identificador. Un mismo proyecto
compila para el teléfono y para el escritorio desde el mismo manifiesto, y dos
bundles que comparten identificador son una sola app para el sistema — un
contenedor, unos ajustes, una partición de llavero.

Dos cosas que conviene saber:

- `an add` solo funciona **dentro de un proyecto**. En el monorepo da error.
- Un proyecto que apunte a Wear OS necesita `android/AndroidManifest.wear.xml`, y
  `an add` no lo crea nunca: escríbelo a mano, o deja que la compilación caiga al
  del shell. `an add android` es lo que crea el directorio donde va.

## Los comandos de compilar y ejecutar

Todos comparten la misma forma.

| Comando | `APP` por defecto | `--device` por defecto | `--release` | `--no-launch` |
|---|---|---|:--:|:--:|
| `an build` | `examples/hello-angular` | — | ✓ | — |
| `an ios` | `examples/hello-angular` | `iPhone 17 Pro` | ✓ | ✓ |
| `an tvos` | `examples/hello-tv` | `Apple TV 4K (3rd generation)` | ✓ | ✓ |
| `an visionos` | `examples/hello-vision` | `Apple Vision Pro` | ✓ | ✓ |
| `an watchos` | `examples/hello-watch` | `Apple Watch Series 11 (46mm)` | ✓ | **—** |
| `an macos` | `examples/controls` | **—** | ✓ | ✓ |
| `an android` | `examples/hello-angular` | **—** | ✓ | ✓ |
| `an wearos` | `examples/hello-wear` | un serial de `adb`, sin valor por defecto | ✓ | ✓ |

Y las banderas que producen algo para un dispositivo o para una tienda. Están en
el subcomando de la propia plataforma y no en un comando aparte, porque cambian
lo que esa compilación *es*, no lo que pasa después:

| Comando | Bandera | Qué sale |
|---|---|---|
| `an ios` | `--physical` | Firmado para un iPhone o iPad conectado, instalado con `devicectl`. `--device` pasa entonces a nombrar el dispositivo y no un simulador. |
| `an ios` | `--archive` | `<Nombre>.xcarchive` y `<Nombre>.ipa`. Implica `--release`. Incompatible con `--physical`. |
| `an android`, `an wearos` | `--sign` | Firmado con el keystore de release en vez de con el de depuración. |
| `an android`, `an wearos` | `--aab` | Un Android App Bundle. Implica `--sign`; no instala nunca. |
| `an macos` | `--sign` | Firma con Developer ID y hardened runtime, en vez de ad hoc. |
| `an macos` | `--notarize` | Eso, enviado a Apple, esperado y grapado. Implica `--sign`. |
| `an macos` | `--dmg` | Un `.dmg`, firmado y notarizado por derecho propio cuando esas banderas están puestas. |

Todas necesitan ajustes que viven en `angular-native.json`, y todas fallan antes
de compilar nada cuando falta una credencial. Ver
[Firma y distribución](/es/guide/signing-and-distribution/), que es además la
página que dice qué hay que conseguir de Apple y de Google.

`--release` y `--sign` están separadas a propósito: la primera va del compilador,
la segunda de la clave. Una compilación para Google Play quiere las dos.

Cuatro asimetrías de la tabla de arriba son reales y no erratas:

- **`an watchos` no tiene `--no-launch`.** Es el único comando de compilación sin
  ella.
- **`an macos` no tiene `--device`** porque no hay simulador: el `.app` corre en
  la máquina que lo compiló.
- **`an android` tampoco tiene `--device`.** El dispositivo se elige por su forma.
  `an wearos` sí la admite, porque un reloj se identifica por un serial de `adb` y
  no por el nombre de un simulador.
- **`an build` se para tras el bundle** e imprime su ruta por la salida estándar.

`--no-launch` construye el artefacto, imprime dónde ha aterrizado y vuelve.

### Elegir dispositivo

En las plataformas de Apple, `--device` es el nombre de un simulador. La búsqueda
parsea `xcrun simctl list devices available -j` como JSON —deliberadamente,
porque `simctl` imprime el udid antes del nombre y un grep devolvería el del
vecino—, filtra los runtimes por familia, prefiere uno arrancado y si no coge la
primera coincidencia. Si no existe ningún runtime de esa familia, lo dice y te da
el comando de descarga en vez de afirmar que no encuentra dispositivo.

En Android el dispositivo se elige preguntando por `ro.build.characteristics`:
`watch` selecciona el dispositivo Wear, cualquier otra cosa el móvil. Con un solo
dispositivo de la forma correcta se usa ese; con ninguno o con varios se te pide
que pases `--device`, y un `--device` de la forma equivocada se rechaza. Esa
última comprobación importa: un APK de reloj se instala en un móvil sin protestar,
y arranca, y pinta, y lo único que no hace es ser una app de reloj.

### Cadena de herramientas por plataforma

| Plataforma | Target de Rust | Crate | Toolchain |
|---|---|---|---|
| iOS · iPadOS | `aarch64-apple-ios-sim` | `an-ios` | estable |
| tvOS | `aarch64-apple-tvos-sim` | `an-ios` | **nightly + `rust-src`** |
| visionOS | `aarch64-apple-visionos-sim` | `an-ios` | **nightly + `rust-src`** |
| watchOS | `aarch64-apple-watchos-sim` | `an-watch` | **nightly + `rust-src`** |
| macOS | `aarch64-apple-darwin` | `an-macos` | estable |
| Android · Wear OS | `aarch64-linux-android` | `an-android` | estable |

Los tres targets de nightly son de tier 3 y no traen `std` precompilada, así que
se construye sobre la marcha con `-Z build-std=std,panic_abort`. Un fallo de
cargo en uno de ellos se vuelve a reportar con los dos comandos de `rustup` que
necesitas, en vez de como un muro de salida de build-std.

Todo lo de Apple pasa por `xcrun swiftc` en una sola invocación sobre
`shells/<plataforma>/Sources` más `shells/shared`, ordenado para que sea
reproducible. No hay ningún `.xcodeproj` en ninguna parte. macOS nombra
`WebKit`, `MapKit`, `AVFoundation` y `AVKit` explícitamente en la línea de
enlazado, porque una `staticlib` de Rust no arrastra sus dependencias de
frameworks — sin eso la app compila, se firma, arranca y después se muere en la
primera vista de esa clase.

Android no lleva Gradle: `aapt2 compile` y `link`, `javac`, `d8`, `zip`,
`zipalign` y `apksigner`, con un keystore de depuración creado en el primer uso.
Las dependencias de Material 3 se resuelven una vez con
`python3 scripts/fetch-android-deps.py` y
`python3 scripts/prepare-android-deps.py`.

### Dónde aterriza cada cosa

La raíz de compilación es `build/` dentro del monorepo y
`<proyecto>/.angular-native/build/` en tu propio proyecto.

```text
build/js/…                        salida de ngc
build/bundle/…/main.js            el bundle
build/ios/<Nombre>.app
build/tvos/<Nombre>TV.app
build/visionos/<Nombre>Vision.app
build/watchos/AngularNativeWatch.app
build/macos/AngularNativeMac.app
build/android/<AppName>.apk       y <AppName>-wear.apk para Wear
build/ios/<Nombre>.xcarchive      --archive
build/ios/<Nombre>.ipa            --archive
build/macos/<Nombre>.dmg          --dmg
build/android/<AppName>-release.apk   --sign
build/android/<AppName>-release.aab   --aab
```

Los artefactos de release llevan un nombre distinto del de los de depuración a
propósito: que una compilación firmada sobrescriba en silencio el APK que llevaba
corriendo tu emulador es la forma en que el fichero equivocado llega a una
tienda.

tvOS y visionOS reciben un sufijo en el nombre y en el id de bundle —`TV`/`.tv`
y `Vision`/`.vision`— para que compilar uno no sobrescriba el `.app` del otro y
para que instalar uno no desinstale al otro.

## `an plugins`

```bash
an plugins [APP] [--platform ios|android]
```

Sin `--platform` lista lo que arrastra la app, una línea por plugin: nombre del
módulo, paquete, qué plataformas cubre y dónde vive. Con `--platform` corre la
misma comprobación de cobertura que corre la compilación, así que sale con
código distinto de cero exactamente cuando la compilación se negaría.

`--platform` solo admite `ios` y `android`. Ver
[Plugins](/es/extending/plugins/) para qué significa cobertura y por qué una
plataforma que falta es un error duro.

## `an dev`

```bash
an dev [APP] [--port <N>] [--device <NOMBRE>]
       [--android] [--wearos] [--watchos] [--tvos] [--visionos] [--macos]
       [--no-launch]
```

Sirve el bundle en `127.0.0.1:8420` —`--port` lo cambia— y vigila los cambios. El
puerto se ocupa *antes* de compilar nada, así que un segundo `an dev` falla de
inmediato en vez de después de dos minutos de compilación.

A la app se le da la URL del servidor en tiempo de compilación: `127.0.0.1` para
todo lo de Apple, el reloj incluido, que comparte la red del Mac, y macOS
incluido, donde la app no está dentro de nada: corre en la máquina que sirve el
bundle. `10.0.2.2` para los emuladores de Android y de Wear.

`--android` gana a `--wearos`, que gana a `--watchos`, `--tvos`, `--visionos` y
`--macos`, y iOS es lo que sale sin ninguna. No están declaradas como mutuamente
excluyentes, así que `an dev --android --tvos` compila Android en silencio.

No existe `an dev --ios`: iOS es lo que sale por defecto.

`--macos` no tiene simulador al que lanzarse: `an dev --macos` mata la ventana
que hubiera abierta y abre otra, igual que `an macos`.

`--device` se comporta como «si no lo has cambiado, usa el valor por defecto de
esta plataforma»: `--watchos` recibe el reloj, `--tvos` el Apple TV, `--visionos`
el visor. `--android` y `--macos` la ignoran del todo — el Mac no tiene
dispositivo que nombrar. `an dev --wearos` la trata como un serial de `adb`.

`[APP]` por defecto es `examples/hello-angular`, salvo con `--wearos`
(`examples/hello-wear`) y `--macos` (`examples/controls`), que es el mismo valor
por defecto que usa el subcomando de cada plataforma y por el mismo motivo: una
pantalla de móvil no se lee en una esfera redonda de 227 puntos, y en el
escritorio `controls` es lo que enseña de un vistazo qué pinta AppKit.

Vigila siempre `<app>/src`, y también `packages/` cuando se ejecuta dentro del
monorepo. Solo cuentan `.ts`, `.js`, `.html` y `.json`, con un debounce de 250 ms.
Una compilación que falla imprime el error y sigue vigilando: no tira el servidor
nunca.

### Qué hace de verdad un guardado

El bundle de desarrollo son dos mitades en un mismo fichero. Todo lo que se
resuelve como paquete pelado —Angular, rxjs, `@angular-native/*`— va en la mitad
de arriba y se marca con un hash; tus módulos relativos van en la mitad de abajo
y se vuelven a evaluar encima.

- Si ha cambiado la **mitad de arriba**, la de abajo no se evalúa siquiera y la
  app se reinicia. En el intérprete solo cabe una copia de Angular, y la que está
  corriendo no se puede reemplazar.
- Si no, los metadatos de Angular se intercambian en su sitio conservando los
  mismos objetos de clase, así que las instancias y las señales sobreviven y te
  quedas en la misma pantalla. Esto se rechaza —y se fuerza un reinicio— cuando
  el conjunto de componentes ha cambiado de tamaño, cuando ha desaparecido una
  clave, o cuando una clase ha pasado de componente a directiva o al revés.
- Una excepción lanzada en cualquier punto de ese camino también fuerza un
  reinicio, en vez de dejarte mirando código actualizado a medias.

En un reinicio en frío las vistas nativas se desmontan, el motor de JS es nuevo,
el árbol está vacío y todas las señales vuelven a su valor inicial. Dos cosas
sobreviven a una recarga en caliente a propósito: las señales de estado caliente,
y el historial del router.

Los shells de Apple hablan con el servidor por WebSocket; el de Android hace
long-polling, porque allí no hay WebSocket de plataforma.

## Entorno

| Variable | Quién la lee | Qué hace |
|---|---|---|
| `AN_HOME` | `an` | Dónde están las fuentes del framework cuando se ejecuta fuera del monorepo. Se mira primero, y se **valida**: apuntarla a algo que no es un SDK es un error, no un recambio. |
| `CARGO_TARGET_DIR` | `an` | Dónde buscar las staticlib compiladas. |
| `ANDROID_HOME`, `ANDROID_SDK_ROOT` | `an` | El SDK de Android, en ese orden, cayendo a `~/Library/Android/sdk`. |
| `AN_IOS_TEAM`, `AN_IOS_IDENTITY`, `AN_IOS_PROFILE` | `an` | Sobrescriben `signing.ios.*`. |
| `AN_MACOS_IDENTITY`, `AN_MACOS_NOTARY_PROFILE` | `an` | Sobrescriben `signing.macos.*`. |
| `AN_ANDROID_KEYSTORE`, `AN_ANDROID_KEY_ALIAS` | `an` | Sobrescriben `signing.android.*`. |
| `AN_ANDROID_KEYSTORE_PASSWORD`, `AN_ANDROID_KEY_PASSWORD` | `an` | Las dos contraseñas. Existen **solo** aquí: el manifiesto nombra la variable, nunca el valor. |
| `AN_BUNDLETOOL` | `an` | Dónde está `bundletool.jar`, para `--aab`. |

Las variables de firma ganan a `angular-native.json`, que es lo que quiere CI y
lo que hace usables esos caminos desde dentro de este repositorio, que no tiene
manifiesto de proyecto.

Fuera del monorepo, `an` sube desde el directorio de trabajo buscando un
`angular-native.json`. El SDK en sí es `AN_HOME` si está puesta, y si no la ruta
horneada en tiempo de compilación — que es lo que hace que
`cargo install --path crates/an-cli` funcione desde cualquier sitio. En cualquier
caso se valida que la raíz contenga el runtime, el empaquetador, los shells y los
crates. Si no se encuentra nada pero el directorio *sí* es un proyecto de Angular,
dice que ejecutes `an init` ahí en vez de reportar que no hay raíz.

## Qué detiene una compilación

Cada una de estas es un error duro con el motivo dentro, no un aviso:

- **Un plugin que no cubre la plataforma que se está compilando.** Se comprueba
  antes de compilar nada. Ver [Plugins](/es/extending/plugins/).
- **Cualquier plugin en watchOS o en macOS.** Ninguno de los dos hosts tiene
  registro de plugins, así que la compilación se niega en vez de publicar una app
  cuyas llamadas serían todas rechazadas en ejecución.
- **Fuentes en Kotlin dentro de un plugin.** El shell de Android es Java
  compilado con `javac`; sin Gradle no hay ningún `kotlinc` al que recurrir, y
  compilar el APK sin esos ficheros sería peor.
- **Dos plugins reclamando el mismo nombre de módulo**, o pidiendo la misma clave
  de `Info.plist`, el mismo entitlement o el mismo `uses-feature` con valores
  distintos.
- **Un `Info.plist` que no concuerda con `angular-native.json`.** Se comprueba
  antes que cargo y swiftc, y nombra el `an add` que te falta.
- **Un `AndroidManifest.xml` que ya no declara `package="dev.angularnative"`.**
- **Una caché vacía de dependencias de Android**, apuntando a
  `scripts/prepare-android-deps.py`.
- **Una contraseña escrita en `angular-native.json`.** Rechazada por su nombre,
  por todos los comandos que corren dentro de un proyecto, no solo por los que
  firman.
- **Un keystore o un perfil que git pueda ver**, tanto si está commiteado como si
  simplemente no está ignorado.
- **Cualquier credencial de firma que falte o esté mal**: que no haya sección
  para la plataforma, un perfil ausente, ilegible, caducado o de otra app, un
  equipo que no concuerda con el perfil, un certificado que no está en el
  llavero, un keystore con la contraseña o el alias equivocados, un `bundletool`
  que no está. Todas ellas antes de llamar a `cargo`.

## Dos cosas que conviene saber antes de topártelas

**Fuera del monorepo, seis invocaciones fallan tal cual se escriben.** `an macos`,
`an tvos`, `an visionos`, `an watchos`, `an wearos` y `an dev --wearos` pasan
siempre su ejemplo por defecto, y fuera del monorepo una ruta de app explícita
que no sea la raíz del proyecto es un error. Pasa el proyecto explícitamente:

```bash
an tvos .
an macos .
```

`an build`, `an ios`, `an android`, `an plugins` y `an dev` a secas no pasan
ningún valor por defecto y no se ven afectados.

**`an dev --watchos` usa por defecto el ejemplo del móvil.** `an watchos` usa
`examples/hello-watch` porque `hello-angular` es ilegible a 205 puntos;
`an dev --watchos` no arrastra ese valor por defecto. Nombra el ejemplo.
