---
title: Biometría y llavero
description: Dos plugins que van juntos — la comprobación biométrica del sistema, y secretos guardados donde el sistema guarda los suyos.
---

Dos plugins escritos fuera del núcleo, y la razón de que exista el
[mecanismo de permisos](/es/extending/plugin-permissions/):

- `@angular-native/plugin-biometrics` — `LAContext` en iOS,
  `android.hardware.biometrics.BiometricPrompt` en Android.
- `@angular-native/plugin-keychain` — Keychain Services en iOS, una clave del
  `AndroidKeyStore` sobre un fichero privado de la app en Android.

Van juntos a propósito. Guardar algo «seguro» que cualquiera puede volver a leer
no protege de nada, y pedir la cara sin atarla a lo que descifra es teatro.
`examples/secrets` usa los dos.

## Biometría

El diálogo lo dibuja el sistema. El plugin no ve nunca una cara ni una huella:
solo cómo acabó.

```ts
const info = await biometrics.availability()
// { status: 'available', kind: 'faceId', detail: 'LAContext.canEvaluatePolicy' }

const resultado = await biometrics.authenticate({
  reason: 'Comprobar que eres tú',
  cancelTitle: 'Ahora no'
})
// { outcome: 'success', kind: 'faceId', detail: 'evaluatePolicy' }
```

### No contesta sí o no

Un booleano aplasta once situaciones en dos, y cuatro de ellas piden que la app
haga algo distinto: mandar a Ajustes, ofrecer el código, esconder el botón, o
volver a intentarlo. Así que `authenticate()` resuelve con un final:

| `outcome` | Qué pasó | Qué suele hacer una app |
|---|---|---|
| `success` | Autenticó. | Seguir. |
| `failed` | La biometría corrió, no reconoció a nadie y el sistema lo dio por terminado. | Ofrecer otro intento. |
| `userCancel` | El usuario cerró el diálogo o pulsó cancelar. | Nada. Ya lo sabe. |
| `userFallback` | Pidió el código del aparato y no estaba permitido. | Volver a llamar con `allowDeviceCredential: true`. |
| `systemCancel` | El sistema quitó el diálogo: la app se fue al fondo, entró una llamada. | Nada, o reintentar luego. |
| `timeout` | Se agotó el tiempo. Solo Android, ver abajo. | Ofrecer otro intento. |
| `noHardware` | Este aparato no tiene sensor. | No enseñar el botón. |
| `notEnrolled` | Hay sensor, pero no hay cara ni huella registrada. | Enlazar a Ajustes. |
| `passcodeNotSet` | El aparato no tiene código, y sin código no hay biometría. | Enlazar a Ajustes. |
| `lockedOut` | Demasiados intentos fallidos. | Ofrecer el código del aparato. |
| `permanentlyLockedOut` | Bloqueado hasta desbloquear el aparato con el código. Solo Android. | Ofrecer el código del aparato. |
| `unavailable` | Hay sensor y el sistema no lo presta. `detail` dice por qué. | Caer a una contraseña. |

**La promesa solo se rechaza cuando el fallo es de quien programó**: un método
que no existe, un `reason` que falta, el plugin sin compilar dentro de la app.
Cancelar, fallar, no tener sensor y estar bloqueado son respuestas, no errores.

`detail` es lo que dijo el sistema: el código y el mensaje de `LAError`, o la
constante de `BiometricManager`. Es diagnóstico, no interfaz: está en inglés,
cambia entre versiones y no le explica nada a un usuario.

### Dónde se separan de verdad las dos plataformas

Ninguna de estas se disimula, porque disimularla es mentir en una de las dos:

- **Qué biometría es.** iOS lo dice (`faceId`, `touchId`, `opticId`); el
  `BiometricManager` de Android contesta si se puede autenticar, no con qué, así
  que ahí `kind` es `unknown`. Rellenarlo con `fingerprint` porque la mayoría de
  los Android lo son sería mentir justo en los que llevan cámara.
- **Bloqueo temporal y permanente.** Android tiene dos códigos; iOS tiene uno
  (`biometryLockout`). Así que `permanentlyLockedOut` solo sale de Android.
  Fingirlo en iOS contando intentos sería inventárselo.
- **Un fallo suelto.** En Android el diálogo sigue en pantalla y el plugin no
  contesta: contestar cerraría la promesa con el diálogo abierto y el resultado
  de verdad no tendría a quién llegar. En iOS el diálogo ofrece reintentar, y lo
  que acaba llegando es `failed`, `userCancel` o un tiempo agotado.
- **Tiempos agotados.** Android manda `BIOMETRIC_ERROR_TIMEOUT`, que se
  convierte en `timeout`. iOS emite un `LAError` cuyo código —el −1003— no está
  en el enumerado público, así que llega como `unavailable` con
  `"LAError -1003: Authentication timed out."` dentro de `detail`. Traducir un
  número que Apple no documenta sería adivinar; el mensaje es exacto.
- **Android necesita Android 10.** `BiometricPrompt` llegó en Android 9, pero
  `BiometricManager.canAuthenticate` —lo único que sabe contestar a
  `availability()` sin enseñar un diálogo— llegó en Android 10 (API 29). Por
  debajo, el plugin contesta `unavailable` con el nivel de API dentro, en vez de
  disimular.

## Llavero

```ts
await keychain.set('token-de-sesion', token, {
  requireBiometrics: true,
  reason: 'Guardar el testigo de sesión protegido con tu cara'
})

const lectura = await keychain.get('token-de-sesion', { reason: 'Enseñar el testigo' })
// { outcome: 'found', value: '…', detail: 'SecItemCopyMatching' }
```

`has()` y `remove()` no preguntan nunca: saber que un elemento existe no es
abrirlo, y el llavero deja tirar un elemento que no se puede leer —menos mal, o
un secreto atado a una huella borrada se quedaría ahí para siempre—.

Una lectura contesta `found`, `notFound`, `denied`, `invalidated` o
`unavailable`. `notFound` y `denied` no son lo mismo, y aplastarlos es el error
clásico de este tipo de API: el primero quiere decir que hay que volver a pedir
la contraseña, y el segundo que el secreto sigue ahí y quien tiene el teléfono
no ha demostrado ser su dueño. Cerrar la sesión en el segundo caso es castigar
al usuario por haber cancelado un diálogo.

### Qué protege de verdad

**iOS — Keychain Services.** Cada secreto es un `kSecClassGenericPassword` con
el identificador del bundle como servicio.

| Pregunta | Respuesta |
|---|---|
| ¿Lo lee otra app? | No. Los grupos del llavero separan las apps. |
| ¿Se lee el fichero sacándolo del aparato? | No. Cifrado en reposo, legible solo tras el primer desbloqueo (`kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`). |
| ¿Sobrevive a desinstalar la app? | **Sí.** iOS no borra los elementos del llavero al quitar una app. Se reinstala y siguen ahí. Si eso importa, hay que borrarlos al cerrar sesión. |
| ¿Viaja en la copia de seguridad? | No. El `…ThisDeviceOnly` lo deja fuera del llavero de iCloud y de las copias cifradas. |
| ¿Sobrevive a un cambio de terminal? | No, por lo mismo. |
| ¿Con `requireBiometrics`? | El elemento queda atado al juego registrado (`.biometryCurrentSet`). Se registra otra cara e iOS tira la clave: el elemento desaparece y una lectura contesta `notFound`. Es lo que impide que alguien con el código del aparato añada su propia cara y se sirva. |
| ¿Un aparato con jailbreak, un depurador, o la app soltando el valor tras leerlo? | **No.** Nada de esto protege un aparato comprometido ni una app descuidada. |

**Android — no hay llavero.** Android no tiene ninguna API que guarde un secreto
por ti; lo que tiene es un *almacén de claves* que guarda claves y no las suelta.
Así que el equivalente se arma con dos piezas: una clave AES-256-GCM generada
dentro del `AndroidKeyStore`, que nunca sale de él, y el secreto cifrado con ella
en un fichero de preferencias `MODE_PRIVATE`.

| Pregunta | Respuesta |
|---|---|
| ¿Lo lee otra app? | No. El fichero es privado y la clave no se puede exportar. |
| ¿La clave está en hardware? | **Depende del teléfono.** TEE o StrongBox donde lo hay, software donde no. `keychain.backing()` lo pregunta y lo dice, porque no es la misma garantía y una app seria puede querer negarse a la floja. |
| ¿Sobrevive a desinstalar la app? | No. Se van el fichero y la clave. |
| ¿Viaja en la copia de seguridad? | El fichero cifrado puede. La clave nunca, así que restaurado en otro aparato no se abre. |
| ¿Sobrevive a un cambio de terminal? | No. |
| ¿Con `requireBiometrics`? | La clave se genera con `setUserAuthenticationRequired(true)` y `setInvalidatedByBiometricEnrollment(true)`, y cada uso pasa por `BiometricPrompt` con un `CryptoObject`. Se registra otra huella y la clave se destruye; lo guardado se queda y ya no se puede abrir nunca: es el final `invalidated`. |
| ¿Un aparato con root? | **Más flojo.** Con hardware detrás la clave sigue sin poderse extraer, pero alguien con root puede pedirle al almacén que la use. Con almacén de software, se la lleva. |

### Una diferencia que hay que saber

**En iOS guardar no pregunta y leer sí. En Android guardar también pregunta.** El
almacén de claves de Android ata la autenticación a *cada uso* de la clave, y
cifrar es un uso. No es un descuido y no está escondido: está escrito en el
contrato de TypeScript, al lado de `requireBiometrics`. Esconderlo obligaría a
cifrar con otra clave, y entonces la protección sería otra.

La otra diferencia es `invalidated`: Android dice que la entrada quedó ilegible
para siempre, mientras que iOS la borra y contesta `notFound`.

## Qué está verificado, y dónde

- **iOS, en el simulador del iPhone 17 Pro, visto funcionando**: qué biometría
  tiene el aparato y si hay algo registrado; una cara que coincide autenticando;
  una cara que no coincide rechazada por el sistema sin enseñar nada; un secreto
  guardado con `SecItemAdd` y leído con `SecItemCopyMatching`; y el
  `NSFaceIDUsageDescription` y el `keychain-access-groups` que hacen posibles las
  dos cosas, fundidos por `an` dentro de la app.
- **No verificable en el simulador**: el simulador **no** aplica el
  `SecAccessControl`. Un elemento guardado con `.biometryCurrentSet` lo devuelve
  `SecItemCopyMatching` sin enseñar Face ID. El control de acceso está puesto en
  el elemento —el código que lo pone es el mismo que corre en un aparato—, pero
  la puerta solo es de verdad sobre hardware con Secure Enclave.
- **Android**: el Java de los dos plugins compila contra `android.jar`, y la
  fusión del manifiesto dice qué permiso y qué característica añade. Nada más.
  **El APK entero no llegó a armarse** —la máquina se quedó sin disco en
  `aapt2 link`— y no se arrancó ningún emulador. Nadie ha visto nada de esto
  funcionar en Android.

### Las capturas

Todavía no hay nada registrado, y la app lo dice en vez de enseñar un botón que
iba a fallar:

![Face ID sin nada registrado](/img/biometrics-not-enrolled.png)

Una cara que coincide — *Features › Face ID › Matching Face* en el simulador:

![Una cara que coincide autentica](/img/biometrics-face-matches.png)

Una que no. El sistema la rechaza, la app sigue esperando y la caja sigue vacía:

![Una cara que no coincide se rechaza](/img/biometrics-face-does-not-match.png)

Guardar un secreto, y volver a leerlo:

![Un secreto guardado con SecItemAdd](/img/keychain-saved.png)

![El secreto leído con SecItemCopyMatching](/img/keychain-read.png)
