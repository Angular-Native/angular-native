import { inject, Injectable } from '@angular/core'
import { NativeModules } from '@angular-native/platform'

/**
 * Qué biometría tiene el aparato.
 *
 * iOS lo dice: `LAContext.biometryType` distingue Touch ID, Face ID y Optic
 * ID. Android no —`BiometricManager` contesta si se puede autenticar, no con
 * qué—, así que ahí lo disponible llega como `unknown`. Es la respuesta
 * honrada: inventar `'fingerprint'` porque la mayoría de los Android lo son
 * sería mentir en los que llevan cámara.
 */
export type BiometryKind = 'faceId' | 'touchId' | 'opticId' | 'unknown' | 'none'

/**
 * Si se puede pedir biometría ahora mismo, y si no, por qué no.
 *
 * Las cuatro razones no son la misma: sin sensor no hay nada que ofrecer y el
 * botón sobra; sin huella registrada el botón sirve pero hay que mandar a
 * Ajustes; bloqueado por intentos se arregla con el código del aparato; y
 * bloqueado del todo ya no se arregla sin desbloquear el teléfono. Una app que
 * las trate igual acaba enseñando «no se pudo» sobre las cuatro.
 */
export type BiometricStatus =
  /** Hay sensor, hay algo registrado y el sistema lo dejaría usar ahora. */
  | 'available'
  /** El aparato no tiene sensor biométrico. */
  | 'noHardware'
  /** Hay sensor, pero nadie ha registrado cara ni huella. */
  | 'notEnrolled'
  /** El aparato no tiene código, y sin código no hay biometría que valga. */
  | 'passcodeNotSet'
  /** Demasiados intentos fallidos. Se recupera desbloqueando con el código. */
  | 'lockedOut'
  /**
   * Bloqueado hasta desbloquear el aparato con el código. Solo Android lo
   * distingue del anterior; iOS los junta en `lockedOut`.
   */
  | 'permanentlyLockedOut'
  /**
   * Hay sensor pero el sistema no lo presta ahora: ocupado, pendiente de una
   * actualización de seguridad, o una versión de Android anterior a la que
   * trae `BiometricPrompt`. `detail` dice cuál.
   */
  | 'unavailable'

export interface BiometricAvailability {
  status: BiometricStatus
  kind: BiometryKind
  /**
   * Lo que dijo el sistema, tal cual: el nombre del error de `LAError` o la
   * constante de `BiometricManager`.
   *
   * **No es texto para enseñar al usuario.** Está en inglés, cambia entre
   * versiones del sistema y no explica nada a quien no escribió la app. Es
   * para el registro y para un informe de fallo.
   */
  detail: string
}

/**
 * Cómo acabó una autenticación.
 *
 * Un booleano aplastaría las once en dos, y las once piden respuestas
 * distintas: `userCancel` no merece ni un aviso —el usuario ya sabe que
 * canceló—, `notEnrolled` merece un enlace a Ajustes, `lockedOut` merece
 * ofrecer el código, y `failed` merece volver a intentarlo.
 */
export type BiometricOutcome =
  /** Autenticó. Es el único que quiere decir que sí. */
  | 'success'
  /**
   * La biometría corrió y no reconoció a nadie, y el sistema dio la sesión por
   * terminada. En iOS llega tras varios intentos seguidos sin acertar; en
   * Android un fallo suelto no termina nada —el diálogo sigue en pantalla— y
   * lo que acaba llegando es `lockedOut`.
   */
  | 'failed'
  /** El usuario cerró el diálogo o pulsó el botón de cancelar. */
  | 'userCancel'
  /**
   * El usuario pidió el código del aparato y no se le había permitido caer
   * ahí (`allowDeviceCredential` en `false`). Solo iOS. Quien quiera atenderlo
   * puede volver a llamar con `allowDeviceCredential: true`.
   */
  | 'userFallback'
  /** El sistema quitó el diálogo: la app se fue al fondo, entró una llamada. */
  | 'systemCancel'
  /**
   * Se agotó el tiempo mirando el sensor.
   *
   * Solo llega desde Android, que tiene `BIOMETRIC_ERROR_TIMEOUT`. iOS también
   * agota el tiempo, pero el `LAError` que devuelve —el −1003— no está en el
   * enumerado público, así que llega como `unavailable` con
   * `"LAError -1003: Authentication timed out."` dentro de `detail`. Traducir
   * un número que Apple no documenta sería adivinar; el mensaje es exacto.
   */
  | 'timeout'
  | 'noHardware'
  | 'notEnrolled'
  | 'passcodeNotSet'
  | 'lockedOut'
  | 'permanentlyLockedOut'
  | 'unavailable'

export interface BiometricResult {
  outcome: BiometricOutcome
  kind: BiometryKind
  /** Como el de {@link BiometricAvailability.detail}: diagnóstico, no interfaz. */
  detail: string
}

export interface BiometricRequest {
  /**
   * Por qué se pide. iOS lo enseña bajo el icono de Face ID; Android, como
   * título del diálogo. Obligatorio en las dos: un diálogo del sistema que no
   * dice para qué es, no lo pasa nadie.
   */
  reason: string
  /** Segunda línea del diálogo. Solo Android tiene sitio para ella. */
  subtitle?: string
  /** El texto del botón de cancelar. Por defecto, el del sistema. */
  cancelTitle?: string
  /**
   * Dejar que el usuario entre con el código, el patrón o la contraseña del
   * aparato si la biometría no va.
   *
   * Cambia lo que se está comprobando: con esto en `true`, `success` quiere
   * decir «quien tiene el aparato sabe abrirlo», no «es la cara registrada».
   * Para desbloquear una pantalla suele estar bien; para autorizar un pago,
   * no.
   */
  allowDeviceCredential?: boolean
}

/**
 * Biometría del sistema: `LAContext` en iOS y `BiometricPrompt` en Android.
 *
 * El diálogo lo dibuja el sistema, no la app: aquí no se pinta nada ni se ve
 * nunca la huella ni la cara. Lo único que cruza la frontera es cómo acabó.
 *
 * Ninguno de los dos métodos rechaza la promesa por algo que le pueda pasar a
 * un usuario. Cancelar, fallar, no tener sensor y estar bloqueado son
 * respuestas, no errores, y llegan en `outcome`. La promesa solo se rechaza
 * cuando el fallo es de quien programó: un método que no existe, un `reason`
 * que falta, o el plugin sin compilar dentro de la app.
 */
@Injectable({ providedIn: 'root' })
export class Biometrics {
  private readonly modules = inject(NativeModules)

  /**
   * Si se puede pedir biometría, sin pedirla: no enseña ningún diálogo.
   *
   * Sirve para decidir si el botón sale y qué pone. Lo que **no** sirve es
   * para dar por hecho el resultado: entre esta llamada y la siguiente el
   * usuario puede haber salido a Ajustes y borrado su huella, así que
   * `authenticate` vuelve a contarlo todo.
   */
  availability(): Promise<BiometricAvailability> {
    return this.modules.call<BiometricAvailability>('biometrics', 'availability')
  }

  /** Enseña el diálogo del sistema y cuenta cómo acabó. */
  authenticate(request: BiometricRequest): Promise<BiometricResult> {
    return this.modules.call<BiometricResult>('biometrics', 'authenticate', request)
  }
}
