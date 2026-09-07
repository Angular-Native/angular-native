---
title: Plugins en el Mac y en el reloj
description: Los dos hosts que antes rechazaban los plugins de plano ahora los cargan — qué cambió en el manifiesto, qué no puede hacer de verdad un reloj, y por qué un entitlement es la diferencia entre una app que arranca y una que muere al lanzarse.
sidebar:
  order: 4
---

Dos de los hosts echaban atrás los plugins. `an macos` paraba la compilación de
cualquier app que dependiera de uno, y `an watchos` hacía lo mismo; los dos lo
decían honestamente, y los dos eran huecos. Están cerrados.

No cambió nada de [el contrato](/es/extending/plugins/). Lo que cambió es que
`angularNative` ahora tiene una sección `macos` y otra `watchos` junto a `ios` y
`android`, que el mismo mecanismo de registro corre en los cuatro hosts, y que
un plugin puede decir **por qué** no puede cubrir una plataforma en lugar de
simplemente no cubrirla.

## El manifiesto

```json
{
  "angularNative": {
    "module": "clipboard",
    "entry": "src/public-api.ts",
    "ios":     { "sources": "native/ios",     "register": "AnClipboardPlugin" },
    "android": { "sources": "native/android", "register": "dev.angularnative.plugins.ClipboardPlugin" },
    "macos":   { "sources": "native/macos",   "register": "AnClipboardPlugin" },
    "watchos": {
      "unsupported": "watchOS has no system pasteboard: UIPasteboard is API_UNAVAILABLE(watchos) and there is nothing standing in for it."
    }
  }
}
```

`macos` y `watchos` aceptan las mismas claves que `ios` — `sources`, `register`,
`plist`, `entitlements` — porque son la misma clase de cosa. Lo nuevo es
`unsupported`.

### `unsupported`, y por qué merece una clave propia

Una compilación que se para diciendo *«plugin-biometrics no cubre watchOS»* no
te dice nada que no pudieras haber leído tú mismo en el `package.json`. Una que
se para diciendo *«un reloj no tiene sensor biométrico, y
`LAPolicyDeviceOwnerAuthenticationWithBiometrics` es `API_UNAVAILABLE(watchos)`»*
te dice lo que solo sabía quien escribió el plugin.

Así que una sección de plataforma puede llevar `unsupported` en lugar de
`sources`, y la negativa lo cita:

```
this app cannot be built for watchOS: 1 of its plugins does not cover it
  · @angular-native/plugin-biometrics (module "biometrics") says it cannot: a watch has
    no biometric sensor, and LocalAuthentication says so in the header: …

These are not unfinished halves: they cannot exist on watchOS. The app has to stop
depending on them for this build, or not be built for watchOS.
```

Ese último párrafo es la otra mitad del asunto. A un plugin que simplemente no
ha llegado a una plataforma se le dice que vaya a escribir la mitad que falta; a
uno que lo ha decidido, no, porque decirle a alguien que escriba un portapapeles
para un dispositivo que no tiene ninguno es pedirle algo imposible.

`sources` y `unsupported` en la misma sección es un error, y un `unsupported`
vacío también: el motivo es todo el campo.

## Qué hacen los tres plugins incluidos

| | macOS | watchOS |
|---|---|---|
| `plugin-clipboard` | `NSPasteboard.general` | **no puede** — `UIPasteboard` es `API_UNAVAILABLE(watchos)` |
| `plugin-biometrics` | `LAContext`, Touch ID, opcionalmente un reloj emparejado | **no puede** — sin sensor; la política de biometría es `API_UNAVAILABLE(watchos)` |
| `plugin-keychain` | Keychain Services, con una salvedad más abajo | **funciona**, menos `requireBiometrics` |

### El portapapeles no es el fichero de iOS bajo un `#if`

`NSPasteboard` no es `UIPasteboard`, y la diferencia no es cosmética. Un
portapapeles guarda varias representaciones de una misma cosa, y `setString`
**añade** una en lugar de reemplazar lo que hay — así que escribir sin llamar
antes a `clearContents()` deja los sabores anteriores en su sitio, y la
siguiente app que pegue puede elegir otro y recuperar el texto viejo.
`clearContents()` es además lo que incrementa el contador de cambios, que es
como se entera de que ha pasado algo cualquier observador de `NSPasteboard` de
la máquina.

### El reloj no tiene biometría, y lo que lo zanja es la cabecera

Merece la pena ser preciso, porque la respuesta obvia casi acierta. watchOS sí
trae LocalAuthentication, `LAContext` existe ahí, y watchOS 9 añadió
`LAPolicyDeviceOwnerAuthenticationWithWristDetection` — una política real que
puedes llamar de verdad, y que responde *«este reloj está en la muñeca en la que
se desbloqueó»*.

Sigue sin ser biometría. `LAPolicyDeviceOwnerAuthenticationWithBiometrics` es
`API_UNAVAILABLE(watchos)`, y también lo son `biometryNotAvailable`,
`biometryNotEnrolled` y `biometryLockout`. Devolver `success` desde la detección
de muñeca bajo un método llamado `authenticate` sería mentir sobre qué se había
verificado, así que el plugin se niega y señala al llavero en su lugar.

### El llavero funciona en un reloj, menos una opción

Security.framework está entero: el mismo `SecItemAdd`, el mismo Secure Enclave,
el mismo almacén privado de la app, y un solo llavero en lugar de los dos del
Mac. Los elementos son `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`, que
en un reloj es una ventana mucho más corta de lo que esa misma constante
significa en un teléfono olvidado en una mesa — un reloj se bloquea en cuanto
sale de la muñeca.

`requireBiometrics` vuelve como `unavailable` y **sin guardar nada**. watchOS sí
tiene `kSecAccessControlUserPresence`, que allí se resuelve al código, y usarlo
habría sido lo fácil: la llamada habría tenido éxito y la app habría creído que
su secreto estaba detrás de una huella. Ese es el único error que nadie ve desde
fuera.

## Entitlements, que en el Mac importan mucho más

En iOS hay un plugin que necesita un entitlement. En el Mac lo necesita casi
cualquiera, porque el App Sandbox deniega por defecto y cada capacidad se pide
por su nombre — la red, el micrófono, un fichero que eligió el usuario, el
llavero. Los declara el plugin y los funde `an`, exactamente igual que las
claves del `Info.plist`.

Hay dos cosas del Mac que conviene saber antes de gastar una tarde en ellas.

**Un Mac tiene dos llaveros.** El llavero de fichero de siempre —
`login.keychain-db`, el que muestra Acceso a Llaveros — es por usuario y lo
comparten todas las apps que lo pidan. El llavero con protección de datos es el
del iPhone: por app, sellado al Secure Enclave, y el único que entiende
`kSecAttrAccessible` o un control de acceso atado a Touch ID. Llegar a él
necesita `keychain-access-groups`. `plugin-keychain` averigua cuál le tocó
escribiendo un elemento desechable al arrancar e informa de la respuesta a
través de `backing()`, en lugar de suponerla.

**Una firma ad-hoc no puede llevar todos los entitlements.** macOS los parte en
dos. Los `com.apple.security.` son restricciones que una app se pone a sí misma,
y puede firmarlas cualquiera. El resto — `keychain-access-groups`,
`application-identifier`, `com.apple.developer.*` — son permisos que el sistema
*concede*, y no va a conceder ninguno por la palabra de una firma que no es de
nadie. No rechaza la capacidad: **rechaza ejecutar el binario**, antes de su
primera línea, con un escueto `Killed: 9`.

Ese es carácter por carácter el mismo síntoma que un
`com.apple.security.cs.allow-jit` ausente, que es sin lo que muere QuickJS, y
distinguir los dos solo desde la consola no es posible. Así que `an macos` deja
fuera de una compilación sin firmar los que dependen de un perfil y lo dice:

```
==> warning: keychain-access-groups (asked for by @angular-native/plugin-keychain)
    is left out of this build.
    An ad-hoc signature cannot carry it —macOS wants a provisioning profile behind
    it— and an .app that carries it anyway is killed the instant it launches, with
    a bare `Killed: 9`.
```

`an macos --sign` le da la de verdad. Mientras tanto todo lo demás sigue
funcionando, y el plugin dice a qué recurrió.

## Qué aspecto tienen los shells

El registro es el de `an-ios`, deliberadamente: el mismo buzón en `an-bridge`,
el mismo cuarteto registrar / despachar / resolver / rechazar, el mismo `pump()`
dentro del fotograma para que un plugin toque AppKit o WatchKit en el hilo
principal. Dos diferencias, y las dos son reales:

- **`attach` recibe un `NSViewController` en el Mac**, donde iOS da un
  `UIViewController`. Esa única línea es por lo que un plugin mantiene un
  directorio de fuentes por plataforma.
- **El protocolo del reloj no tiene `attach` en absoluto.** Su shell es una `App`
  de SwiftUI y no hay ningún view controller por ninguna parte. Un plugin que
  necesite poner algo en pantalla no puede escribirse contra él, y un sustituto
  que no entregara nada solo escondería eso hasta tiempo de ejecución.

## Comprobarlo

`scripts/check-plugins-hosts.sh` cubre todo lo anterior sin dispositivo: las
negativas y cómo están redactadas, el registro generado, el entitlement que
tiene que estar en la firma y el que no debe estar, y luego una pulsación real
en una ventana de Mac en marcha con `NSPasteboard` escrito y leído de vuelta.

La ejecución de punta a punta del reloj está detrás de `AN_CHECK_WATCH_APP=1`,
porque `aarch64-apple-watchos-sim` es un target de nivel 3 y compilar `std`
desde las fuentes lleva minutos. `examples/watch-secrets` hace todo su viaje de
ida y vuelta al arrancar y escribe el veredicto en su primera línea —guardar,
leer de vuelta, borrar, y después confirmar que `requireBiometrics` se
rechaza— porque tocar un simulador de reloj necesita el propio ratón del Mac
sobre la ventana del Simulator, y una comprobación no siempre tiene un
escritorio que tomar prestado.
