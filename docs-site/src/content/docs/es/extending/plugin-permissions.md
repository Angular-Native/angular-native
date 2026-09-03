---
title: Permisos que necesita un plugin
description: Cómo declara un plugin claves del Info.plist, derechos y entradas del AndroidManifest, y cómo los funde `an`.
sidebar:
  order: 2
---

Un plugin que habla con la cámara, el micrófono, el llavero o Face ID no
necesita solo código. Necesita que la app donde se instala **declare** algo, y
hasta ahora no tenía forma de decirlo: quien instalaba el plugin tenía que
saberse esa lista de memoria.

El peor caso es Face ID. Sin `NSFaceIDUsageDescription` en el `Info.plist`, iOS
no avisa y no devuelve un error: mata el proceso en cuanto se evalúa la
política. Desde fuera la app se cierra sola.

Así que el plugin declara lo que necesita en su propio `package.json`, y `an` lo
funde al armar el `.app` y el APK.

## Qué puede declarar

```jsonc
{
  "angularNative": {
    "module": "biometrics",
    "ios": {
      "sources": "native/ios",
      "register": "AnBiometricsPlugin",

      // Claves que se añaden al Info.plist de la app.
      "plist": {
        "NSFaceIDUsageDescription": "Para comprobar que eres tú antes de enseñar lo que hay guardado."
      },

      // Derechos que se enlazan dentro del binario. `$(BUNDLE_ID)` se cambia
      // por el identificador de la app, que el plugin no puede saber.
      "entitlements": {
        "keychain-access-groups": ["$(BUNDLE_ID)"]
      }
    },
    "android": {
      "sources": "native/android",
      "register": "dev.angularnative.plugins.BiometricsPlugin",

      "manifest": {
        "uses-permission": ["android.permission.USE_BIOMETRIC"],
        "uses-feature": { "android.hardware.fingerprint": false }
      }
    }
  }
}
```

Lo demás no cambia: la app sigue declarando el plugin como dependencia y ya.

### Valores admitidos

`plist` y `entitlements` aceptan cadenas, booleanos, números y listas de
cadenas. Un diccionario anidado para el build y lo dice: fundirlo bien no está
hecho, y fundirlo mal sería peor que negarse. Las claves tienen que ser de
primer nivel y sin puntos, porque la clave viaja a `plutil -replace`, que trata
el punto como separador de camino.

`manifest` acepta hoy dos secciones:

| Sección | Forma | Se convierte en |
|---|---|---|
| `uses-permission` | lista de nombres | `<uses-permission android:name="…" />` |
| `uses-feature` | nombre → `required` | `<uses-feature android:name="…" android:required="…" />` |

Cualquier otra cosa para el build en vez de ignorarse.

## Cómo se funde

`an` lee todos los plugins de los que depende la app y funde lo que piden. El
`Info.plist` y el `AndroidManifest.xml` del proyecto **no se tocan nunca**: son
de quien los escribió. Las copias fundidas se escriben en `build/`.

Cada clave que entra se dice:

```console
$ an ios examples/secrets
==> plugin biometrics (1 fuentes Swift)
==> plugin keychain (1 fuentes Swift)
==> derechos: keychain-access-groups (de @angular-native/plugin-keychain)
==> Info.plist: NSFaceIDUsageDescription (de @angular-native/plugin-biometrics)
```

### Dos plugins que piden lo mismo

**Misma clave, mismo valor** no es un choque. Dicen lo mismo, así que se escribe
una vez. Es el caso normal: el plugin de biometría y el del llavero necesitan
los dos `NSFaceIDUsageDescription`, y los dos traen la misma frase.

**Misma clave, valores distintos** para el build. No hay forma honrada de
elegir: quedarse con el primero por orden de dependencia o por orden alfabético
sería decidir en silencio qué frase lee el usuario en un diálogo del sistema.

```console
$ an plugins mi-app --platform ios
Error: dos plugins piden la clave "NSFaceIDUsageDescription" del Info.plist con valores distintos:
  · @acme/plugin-face
      "Para desbloquear la app."
  · @otro/plugin-vault
      "Para abrir la caja fuerte."

Solo puede quedar uno, y elegirlo por orden sería decidir en silencio algo que se ve en pantalla.
O los dos plugins se ponen de acuerdo, o la app se queda con uno de los dos.
```

Los permisos de Android no pueden chocar: un permiso no tiene valor, así que
pedirlo dos veces es pedirlo una. Las características sí, porque
`android:required` es un valor, y *obligatoria* y *opcional* no son la misma
petición. Eso también para el build.

La comprobación corre dentro de `an plugins <app> --platform ios`, antes de
invocar a `swiftc`, así que un choque sale en un segundo en vez de en medio
minuto.

### Cuando la app ya lo declara

Si el `Info.plist` o el `AndroidManifest.xml` de la app ya declaran la clave,
gana la app: es su fichero y su autor es un dueño sin discusión. Pero no en
silencio: la línea que dice de quién era el valor que se ignora sale por la
salida de error.

## Los derechos de iOS: por qué se enlazan y no se firman

Los derechos son lo que le permite a una app **pedirle** algo al sistema. Sin
`keychain-access-groups` ni `application-identifier`, `SecItemAdd` contesta
`errSecMissingEntitlement` —el −34018, «el cliente no tiene ninguno de los
dos»— porque la app no pertenece a ningún grupo del llavero y no hay dónde
guardar.

Para el simulador, los derechos van **dentro del binario**, en una sección
`__TEXT,__entitlements` que `an` le pide al enlazador:

```
-Xlinker -sectcreate -Xlinker __TEXT -Xlinker __entitlements -Xlinker <fichero>
```

Firmarlos no vale: ni ad hoc ni con una identidad de desarrollo de verdad.
`keychain-access-groups` es un derecho restringido, y macOS se niega a ejecutar
un binario que lo lleve en la firma sin un perfil de aprovisionamiento que lo
respalde. El síntoma es que la app deja de arrancar, con un «request denied by
SBMainWorkspace» que no menciona los derechos por ninguna parte. Xcode hace
exactamente esto mismo para el simulador.

`application-identifier` lo pone `an` y no el plugin: un plugin no sabe en qué
app va a acabar. Es también lo que le da valor al `$(BUNDLE_ID)`.

## Lo que falta

- **Aparatos de verdad.** `an` instala en el simulador, y lo de la sección del
  binario es cosa del simulador. Un aparato necesita identidad de firma y perfil
  de aprovisionamiento, y no está.
- **Diccionarios anidados en el plist.** `NSAppTransportSecurity` y compañía no
  se pueden aportar todavía. El build lo dice en vez de fundirlos a medias.
- **Recursos.** Un plugin sigue sin poder traer un icono, un sonido o un
  `.strings`.
- **Todo lo que no sea `uses-permission` ni `uses-feature`** en Android: ni
  `<queries>`, ni `<provider>`, ni atributos en `<application>`.
