---
title: Firma y distribución
description: Poner una compilación en un dispositivo real y en una tienda — dónde viven los ajustes, qué tienes que conseguir tú de Apple y de Google, y cuáles de estos caminos no se han ejecutado nunca.
sidebar:
  order: 11
---

Todo lo demás de esta documentación corre en un simulador o en un emulador,
firmado con una clave que no significa nada. Esta página va de la otra mitad:
una compilación que se instala en un teléfono que alguien te pasa, y un fichero
que una tienda acepta.

Esa mitad no se puede hacer por ti. Un certificado se le emite a una persona, un
perfil de aprovisionamiento lista dispositivos por su número de serie, y una
clave de subida es un secreto que generas y que después no puedes perder nunca.
Lo que `an` sí puede hacer es saber dónde están esas cosas, comprobarlas antes
de gastar dos minutos compilando, y decir cuál falta en una frase sobre la que
puedas actuar. Lo que no puede es tenerlas.

## Lo que existe

| Comando | Qué sale | Necesita |
|---|---|---|
| `an ios --physical` | El `.app`, firmado, instalado en un iPhone o iPad conectado | Un certificado de desarrollo y un perfil que liste ese dispositivo |
| `an ios --archive` | `<Nombre>.xcarchive` y `<Nombre>.ipa` | Un certificado de distribución y un perfil de App Store o ad-hoc |
| `an android --sign` | `<Nombre>-release.apk` | Un keystore que generas tú |
| `an android --aab` | `<Nombre>-release.aab` para Google Play | El mismo keystore, más `bundletool` |
| `an wearos --sign`, `an wearos --aab` | Lo mismo, para un dispositivo Wear OS | El mismo keystore |
| `an macos --sign` | El `.app`, firmado con Developer ID, hardened runtime | Un certificado de Developer ID |
| `an macos --notarize` | Lo mismo, notarizado y grapado | Eso, más credenciales de notarytool |
| `an macos --dmg` | `<Nombre>.dmg` | Nada, pero mira más abajo |

Y lo que no existe: **tvOS, visionOS, watchOS y Wear OS no tienen camino propio
de dispositivo ni de tienda.** Compilan para sus simuladores y ahí se paran.
Tampoco hay ningún comando de subida — ningún `an` habla con App Store Connect
ni con la Play Console, y nada de aquí manda una app a revisión.

## Dónde viven los ajustes

En `angular-native.json`, junto a `app` y `platforms`, en un objeto `signing`
indexado por plataforma:

```json
{
  "app": {
    "name": "MyApp",
    "bundleId": "com.example.myapp",
    "entry": "src/main.native.ts"
  },
  "platforms": ["ios", "android"],
  "signing": {
    "ios": {
      "team": "ABCDE12345",
      "identity": "Apple Development",
      "profile": "ios/profiles/development.mobileprovision"
    },
    "macos": {
      "identity": "Developer ID Application",
      "notaryProfile": "an-notary",
      "profile": "macos/profiles/developer-id.provisionprofile"
    },
    "android": {
      "keystore": "../secrets/release.keystore",
      "keyAlias": "upload",
      "storePasswordEnv": "AN_ANDROID_KEYSTORE_PASSWORD",
      "keyPasswordEnv": "AN_ANDROID_KEY_PASSWORD"
    }
  }
}
```

| Clave | Qué es |
|---|---|
| `ios.team` | El Team ID de diez caracteres. Opcional: sin él se usa el del propio perfil, que es la autoridad de todos modos. |
| `ios.identity` | Un prefijo del nombre del certificado, tal como lo imprime `security find-identity -v -p codesigning`. Por defecto `Apple Development` para `--physical` y `Apple Distribution` para `--archive`. |
| `ios.profile` | Ruta al `.mobileprovision`, relativa al proyecto. |
| `macos.identity` | Lo mismo, y casi siempre es `Developer ID Application`. |
| `macos.notaryProfile` | El **nombre** de un perfil de llavero de `notarytool`. No una credencial. |
| `macos.profile` | Ruta a un `.provisionprofile` de macOS, relativa al proyecto. Sólo hace falta cuando un plugin pide un derecho restringido — ver abajo. |
| `android.keystore` | Ruta al keystore, relativa al proyecto. |
| `android.keyAlias` | Qué clave de dentro. |
| `android.storePasswordEnv` | El **nombre de una variable de entorno**. Por defecto `AN_ANDROID_KEYSTORE_PASSWORD`. |
| `android.keyPasswordEnv` | Lo mismo, por defecto `AN_ANDROID_KEY_PASSWORD`, y la contraseña del almacén cuando esa variable no está puesta. |

### Los derechos de un plugin en el Mac

Distribuir no necesita perfil de aprovisionamiento: un `.app` se firma con un
certificado Developer ID y se notariza, y ninguno de los dos pasos usa uno. **Un
derecho sí.** macOS los parte en dos:

- Los `com.apple.security.` —las relajaciones del hardened runtime, el sandbox—
  son restricciones que la app se pone a sí misma. Los puede firmar cualquiera,
  ad hoc incluido, y `an macos` siempre escribe los dos sin los que el motor no
  vive.
- Todo lo demás —`keychain-access-groups`, `application-identifier`,
  `com.apple.developer.*`, los grupos de app— son permisos que concede el
  *sistema*, y sólo los concede a una app que lleva un perfil que los autoriza.
  Un certificado por sí solo no basta.

Un `.app` que lleva uno de los segundos sin el perfil **muere en cuanto
arranca**: `Killed: 9`, nada en el log sobre derechos, y desde fuera es un
cuelgue de la app. Así que:

| Lo que ejecutas | Qué le pasa a `keychain-access-groups` |
|---|---|
| `an macos` | Se queda fuera, con un aviso que nombra el plugin que lo pedía. El plugin se repliega a lo que puede hacer sin él y lo dice en tiempo de ejecución. |
| `an macos --sign` sin `macos.profile` | La compilación se detiene, nombrando el plugin y el derecho. No hay artefacto, porque el único disponible sería uno que no arranca. |
| `an macos --sign` con un perfil que no lo lleva | El mismo rechazo, nombrando también el perfil. No se puede firmar nada que el perfil no lleve. |
| `an macos --sign` con un perfil que lo lleva | Se escribe en la firma, y el perfil entra en el bundle como `Contents/embedded.provisionprofile`. |

El perfil y el certificado tienen que ser de la misma cuenta, y eso se comprueba
después de firmar: a `codesign` le da igual, y el sistema lo dice matando la app.

El perfil sale del
[portal de desarrollador](https://developer.apple.com/account/resources/profiles/list)
—un perfil de macOS para este app id, con la capacidad activada— y necesita el
Apple Developer Program de pago. El app id tiene que ser el del bundle del Mac:
el `bundleId` del proyecto con `.mac` al final.

No hay `macos.team`, a propósito. Nada de esa plataforma lo necesita: el
certificado lleva el equipo y `notarytool` lo saca del perfil del llavero. Una
clave en el manifiesto que no cambia nada es una clave con la que alguien pierde
una tarde intentando acertar.

### El entorno gana

Cada ajuste tiene una variable de entorno que sobreescribe el fichero:

| Variable | Sobreescribe |
|---|---|
| `AN_IOS_TEAM` | `signing.ios.team` |
| `AN_IOS_IDENTITY` | `signing.ios.identity` |
| `AN_IOS_PROFILE` | `signing.ios.profile` |
| `AN_MACOS_IDENTITY` | `signing.macos.identity` |
| `AN_MACOS_NOTARY_PROFILE` | `signing.macos.notaryProfile` |
| `AN_MACOS_PROFILE` | `signing.macos.profile` |
| `AN_ANDROID_KEYSTORE` | `signing.android.keystore` |
| `AN_ANDROID_KEY_ALIAS` | `signing.android.keyAlias` |
| `AN_BUNDLETOOL` | Dónde está `bundletool.jar` |

Eso es lo que usa la CI, donde el keystore se descodifica en un directorio
temporal y la ruta es distinta en cada ejecución. Es además la única fuente que
hay cuando `an` corre dentro de este repositorio, que no tiene manifiesto de
proyecto en absoluto.

## Nunca comitees una credencial

**Ninguna contraseña va en `angular-native.json`.** Ese fichero se comitea; una
contraseña en él es una contraseña en tu historial. Así que el manifiesto guarda
el *nombre* de una variable de entorno, y una literal se rechaza por su nombre:

```text
angular-native.json: signing.android.storePassword is a secret, and this file is committed.
Name an environment variable instead of holding the value:
    "storePasswordEnv": "AN_SOMETHING_PASSWORD"
and export the value where the build runs. If this password has already been
pushed, it has to be changed.
```

Esa comprobación corre al leer el manifiesto, que es lo que hacen todos los
comandos de un proyecto — no solo los de firma. Un secreto que solo nota el
comando que lo necesita es un secreto que se pasa meses en un repositorio.

Los ficheros también se comprueban. Si el keystore o el perfil están dentro del
proyecto y git no los está ignorando, la compilación se para antes de compilar
nada:

```text
release.keystore is the release keystore, and git is not ignoring it: the next
`git add .` commits it.
Add it to .gitignore first:
    echo 'release.keystore' >> .gitignore
```

y si ya está comiteado, el mensaje dice que la credencial en sí está gastada —
porque lo está. La tiene cualquiera que haya clonado el repositorio alguna vez.

`an init` mete `*.keystore`, `*.jks`, `*.p12` y `*.mobileprovision` en el
`.gitignore` del proyecto exactamente por esto. El mejor sitio para un keystore
sigue siendo fuera del proyecto del todo.

## Lo que tienes que conseguir: Apple

Nada de esto se puede automatizar, y todo se hace una vez.

<!-- Escrito como prosa y no como un script porque cada uno de estos pasos es
     una página web que cambia de aspecto dos veces al año. Lo que no cambia es
     lo que acabas teniendo en la mano. -->

1. **Un Apple ID, y para casi todo una membresía de pago.** Un Apple ID gratuito
   puede firmar una compilación en tu propio dispositivo — Xcode emite un
   certificado de siete días para eso. Todo lo demás necesita el Apple Developer
   Program, que son 99 $ al año: TestFlight, la App Store, y un certificado de
   Developer ID para distribuir una app de Mac fuera de la tienda. No hay vuelta
   que darle y no hay una capa gratuita de eso.

2. **Un certificado, en el llavero de este Mac.** Para `--physical`, uno de
   *Apple Development* — Xcode ▸ Settings ▸ Accounts ▸ Manage Certificates ▸
   **+** ▸ Apple Development crea uno y lo instala en un solo paso. Para
   `--archive`, uno de *Apple Distribution*, desde
   [developer.apple.com/account/resources/certificates](https://developer.apple.com/account/resources/certificates/list).
   Para `an macos --sign`, uno de *Developer ID Application*, desde la misma
   página.

   Un `.cer` descargado de esa página es solo la mitad: lleva el certificado
   público, y no sirve de nada sin la clave privada que se generó cuando hiciste
   la petición. Si te mudas a un Mac nuevo, exporta un `.p12` del llavero del
   viejo — ese es el fichero que lleva las dos cosas.

3. **Un App ID**, en
   [developer.apple.com/account/resources/identifiers](https://developer.apple.com/account/resources/identifiers/list),
   que coincida exactamente con tu `app.bundleId`. Uno con comodín
   (`com.example.*`) sirve para desarrollo y no se acepta para nada que use
   notificaciones push o iCloud.

4. **El UDID del dispositivo**, en
   [developer.apple.com/account/resources/devices](https://developer.apple.com/account/resources/devices/list).
   `xcrun devicectl list devices` lo imprime con el teléfono enchufado.

5. **Un perfil de aprovisionamiento**, en
   [developer.apple.com/account/resources/profiles](https://developer.apple.com/account/resources/profiles/list),
   que ate las tres cosas: un perfil de iOS App Development para `--physical`,
   que liste ese dispositivo; uno de App Store para `--archive`. Descárgalo y
   ponlo donde apunte `signing.ios.profile`.

6. **Para `an macos --notarize`, credenciales de notarytool.** Una contraseña
   específica de app desde [account.apple.com](https://account.apple.com) ▸
   Sign-In and Security ▸ App-Specific Passwords, guardada una vez en el
   llavero:

   ```bash
   xcrun notarytool store-credentials an-notary \
       --apple-id tu@ejemplo.com \
       --team-id ABCDE12345 \
       --password xxxx-xxxx-xxxx-xxxx
   ```

   `an-notary` es entonces lo que nombra `signing.macos.notaryProfile`. La
   contraseña se queda en el llavero y no llega nunca al proyecto.

7. **En el dispositivo: modo desarrollador activado.** Ajustes ▸ Privacidad y
   seguridad ▸ Modo desarrollador. El dispositivo se reinicia. Sin eso
   `devicectl` rechaza la instalación, y el rechazo no dice por qué.

## Lo que tienes que conseguir: Google

Menos, y nada de ello cuesta nada hasta que publicas.

1. **Un keystore, que generas tú.** No lo emite nadie. Es un fichero, guarda una
   clave, y Google Play ata tu app a ella para siempre — una app ya publicada no
   se puede actualizar con otra clave sin pedirle a Google que la reinicie.

   ```bash
   keytool -genkeypair -v -keystore ~/secrets/myapp-release.keystore \
       -alias upload -keyalg RSA -keysize 2048 -validity 10000
   ```

   Guárdalo donde el proyecto no llegue. Haz copia de seguridad. Perderlo es el
   único error de esta página sin una recuperación limpia.

2. **`bundletool`, para `--aab`.** No forma parte del SDK de Android — Gradle lo
   arrastra como dependencia, y aquí no hay Gradle.

   ```bash
   python3 scripts/fetch-android-deps.py
   ```

   lo deja en `vendor/android/tools/bundletool.jar`. O apunta `AN_BUNDLETOOL` a
   una copia que ya tengas.

3. **Una cuenta de Play Console**, 25 $ una vez, en
   [play.google.com/console](https://play.google.com/console). Creas la app ahí,
   y la primera subida es donde Play ofrece **Play App Signing**: acéptalo.
   Google guarda entonces la clave que firma lo que instalan los usuarios, y la
   clave de tu keystore pasa a ser la de *subida* — la que demuestra que el
   bundle vino de ti. Si pierdes una clave de subida te pueden emitir otra; no
   hay equivalente para la clave de firma de la app, y por eso Play prefiere
   guardarla ella.

4. **Un código de versión que suba.** `android:versionCode` en
   `android/AndroidManifest.xml`. Play rechaza un bundle cuyo código de versión
   ya ha visto, y lo rechaza al subirlo — después de haberlo compilado todo.
   `an android --aab` comprueba que hay uno antes de compilar nada, pero no puede
   saber qué números has usado ya.

## Los comandos

### `an ios --physical`

```bash
an ios --physical                    # el único dispositivo enchufado
an ios --physical --device "iPhone de Jane"
an ios --physical --no-launch        # compilar y firmar, instalar a mano
```

Compila para `aarch64-apple-ios` en lugar del target del simulador, incrusta el
perfil en el bundle como `embedded.mobileprovision`, y firma con los entitlements
**que concede el perfil**. Esa última parte no es un detalle: el sistema no le da
a una app nada que su perfil no lleve, así que unos entitlements firmados encima
de uno producen una app que se instala y muere en el instante en que se lanza.
Las claves que piden los plugins se funden y el perfil gana todas las colisiones.

Con un dispositivo conectado se usa ese; con varios se te pide que nombres uno.

### `an ios --archive`

```bash
an ios --archive
```

Implica `--release`. Escribe `build/ios/<Nombre>.xcarchive` —la app bajo
`Products/Applications`, los símbolos de depuración bajo `dSYMs`— y
`build/ios/<Nombre>.ipa`, cuya ruta imprime.

No hay ninguna bandera `--method`. Si ese `.ipa` puede ir a TestFlight, a la App
Store o a un puñado de dispositivos ad-hoc lo decidieron el certificado y el
perfil con los que lo firmaste; una bandera aquí solo sería un segundo sitio
donde decir lo mismo y un segundo sitio donde equivocarse.

Subirlo no es parte de esto. Abre el `.xcarchive` en el Organizer de Xcode, o
usa `xcrun altool --upload-app`.

### `an android --sign` y `--aab`

```bash
export AN_ANDROID_KEYSTORE_PASSWORD='…'
an android --sign --release          # un APK firmado para publicar
an android --aab --release           # el bundle que acepta Play
```

`--release` va del compilador y `--sign` va de la clave. Están separadas porque
son cosas separadas, y una compilación para Play quiere las dos. `--aab` implica
`--sign`: Play no acepta nada firmado con una clave de depuración.

Los artefactos se llaman distinto —`<Nombre>-release.apk`, `<Nombre>.apk`— para
que una compilación de publicación no pueda pisar en silencio el APK de
depuración que lleva corriendo tu emulador.

Un bundle no es un APK con otra extensión. Su manifiesto y sus recursos son
protobuf, su disposición es un zip de módulo, `bundletool` lo ensambla y
`jarsigner` lo firma, porque `apksigner` se niega. Todo eso es problema de `an`,
no tuyo; lo tuyo es el keystore y el código de versión.

### `an macos --sign`, `--notarize`, `--dmg`

```bash
an macos . --sign                    # Developer ID + hardened runtime
an macos . --notarize                # eso, enviado, esperado y grapado
an macos . --notarize --dmg          # y empaquetado en un .dmg firmado y notarizado
```

Sin `--sign`, `an macos` firma ad hoc: corre en la máquina que lo compiló y en
ninguna otra. `--dmg` por su cuenta sigue funcionando y lo dice en voz alta — un
`.dmg` de una compilación ad-hoc es una buena manera de comprobar el
empaquetado y no sirve de nada como descarga.

Firmar enciende el **hardened runtime**, que exige la notarización, y que
prohíbe mapear memoria ejecutable y escribible — lo primero que hace un motor de
JavaScript. Así que la compilación declara `com.apple.security.cs.allow-jit`.
Sin eso la app muere al arrancar con un `Killed: 9` que no menciona ningún
entitlement por ninguna parte; es el fallo más confuso de esta página, y está
resuelto en lugar de dejado en tus manos.

Grapar no es opcional y es fácil saltárselo: sin ello la app está notarizada
pero el tique vive en los servidores de Apple, así que a la primera persona que
la abra sin conexión se le dice que la app no se puede abrir, sin nada que lo
distinga de una app que nunca se notarizó. `--notarize` grapa.

El `.dmg` se firma y se notariza por derecho propio cuando `--notarize` está
puesto, porque la imagen es el fichero que se descarga. Un `.dmg` que lleve una
app perfectamente notarizada sigue siendo una descarga sin firmar.

## Cuando falta algo

Todos estos se paran **antes de compilar nada**, y nombran la cosa:

- una plataforma sin sección `signing` — y enseña el bloque que pegar y las
  variables de entorno que hacen lo mismo;
- un perfil que no está, o que no es un perfil;
- un perfil caducado, con la fecha;
- un perfil de otra app, con los dos ids de bundle;
- un equipo que no coincide con el del perfil;
- un certificado que no está en el llavero — listando los que sí están;
- un keystore que no está, con la línea de `keytool` que crea uno;
- un keystore cuya contraseña está mal, o cuyo alias no está dentro: el keystore
  se abre por adelantado, así que las dos cosas llegan en un tercio de segundo
  en lugar de después de dos minutos en `apksigner`;
- una variable de entorno que nadie exportó, por su nombre;
- `bundletool` ausente, nombrando los tres sitios donde se buscó;
- un `devicectl` demasiado viejo, y el Xcode que tiene uno.

Y cuando una herramienta se niega igualmente, se repite lo que dijo con las dos
cosas que nunca menciona: qué identidad se usó, y qué fichero se estaba
firmando.

## Lo que se ha ejecutado de verdad

Ser preciso con esto importa aquí más que en ninguna otra parte de estos
documentos, porque un camino de firma que no se ha ejecutado nunca tiene
exactamente el mismo aspecto que uno que sí.

**Ejercitado de punta a punta por `scripts/check-signing.sh`, en una máquina sin
cuenta de Apple:** todo el camino de publicación de Android. Un keystore
generado por la comprobación, un APK firmado con él y abierto después para
confirmar de quién es el certificado que lleva, un bundle ensamblado, firmado y
descomprimido para confirmar su disposición. También la lectura y la
comprobación de perfiles de aprovisionamiento — la comprobación construye
perfiles reales envueltos en CMS con `openssl` y un certificado desechable, así
que la caducidad, el id de app y el equipo se validan contra ficheros de verdad.

**Ejercitado hasta la credencial:** todo lo demás de Apple. La comprobación
confirma que `an ios --physical` y `--archive` pasan el perfil y se paran en
`codesign` nombrando la identidad que querían, y que `an macos --notarize`
informa del certificado y del perfil de notaría juntos. La invocación de
`codesign` en sí no se ejecuta, porque no hay ningún certificado con el que
ejecutarla.

**Nunca ejecutado, por nadie, en ningún sitio:** `devicectl` instalando en un
iPhone físico, `notarytool` enviando a Apple, `stapler`, y cualquier subida a
una tienda. Eso se escribió a partir de la documentación de Apple. Si eres la
primera persona con una cuenta de Apple Developer que apunta esto a un
dispositivo real, espera encontrarte algo — y las variantes con `--no-launch`
compilan y firman el artefacto de todos modos, así que puedes arrastrarlo a la
ventana de Devices de Xcode y comparar.
