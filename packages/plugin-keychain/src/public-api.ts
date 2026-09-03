import { inject, Injectable } from '@angular/core'
import { NativeModules } from '@angular-native/platform'

/**
 * Cómo acabó una lectura.
 *
 * `notFound` y `denied` no son lo mismo y aplastarlos en «no hay valor» es el
 * error clásico de este tipo de API: la primera quiere decir que hay que pedir
 * la contraseña otra vez, y la segunda que el secreto sigue ahí y quien está
 * delante no ha demostrado ser su dueño. Cerrar la sesión en el segundo caso
 * sería castigar al usuario por haber cancelado un diálogo.
 */
export type KeychainReadOutcome =
  /** Estaba, y ahí va. */
  | 'found'
  /** No hay nada guardado con esa clave. */
  | 'notFound'
  /**
   * Está guardado y no se ha podido abrir: el usuario canceló la biometría, no
   * la superó, o está bloqueado por intentos. El valor sigue donde estaba.
   */
  | 'denied'
  /**
   * Estaba guardado y ya no se puede descifrar nunca: quien tiene el aparato
   * ha cambiado las huellas o caras registradas, y una entrada protegida por
   * biometría se ata al juego que había cuando se guardó.
   *
   * Solo Android lo distingue. iOS borra el elemento cuando eso pasa, así que
   * ahí la misma situación llega como `notFound`.
   */
  | 'invalidated'
  /** No hay dónde guardar esto: sin sensor, sin código de aparato, sin almacén. */
  | 'unavailable'

export interface KeychainRead {
  outcome: KeychainReadOutcome
  /** El secreto, y solo con `found`. En cualquier otro caso es `null`. */
  value: string | null
  /** Lo que dijo el sistema. Diagnóstico, no texto para enseñar. */
  detail: string
}

export type KeychainWriteOutcome = 'saved' | 'denied' | 'unavailable'

export interface KeychainWrite {
  outcome: KeychainWriteOutcome
  detail: string
}

export interface KeychainOptions {
  /**
   * Exigir biometría —o el código del aparato— para volver a leerlo.
   *
   * **Las dos plataformas no piden lo mismo en el mismo momento.** En iOS
   * guardar no pregunta nada y leer enseña Face ID o Touch ID. En Android la
   * clave que cifra es de un solo uso autenticado, así que **guardar también
   * pide la huella**. No es un descuido: es lo que hay: el almacén de claves de
   * Android ata la autenticación a cada uso de la clave, sea cifrar o
   * descifrar. Quien no quiera esa diferencia, que no use esta opción.
   */
  requireBiometrics?: boolean
  /**
   * Lo que enseña el diálogo del sistema. Obligatorio si
   * `requireBiometrics` es `true`: un diálogo que no dice para qué es no lo
   * pasa nadie.
   */
  reason?: string
}

/** Lo que de verdad guarda los secretos en este aparato. */
export interface KeychainBacking {
  platform: 'ios' | 'android'
  /**
   * Si la clave vive en hardware al que ni el sistema puede asomarse: el
   * Secure Enclave en iOS, el TEE o el StrongBox en Android.
   *
   * En Android hay aparatos que no lo tienen y el almacén de claves es software.
   * Ahí sigue habiendo cifrado y la clave sigue sin salir del sistema, pero un
   * atacante con root se la puede llevar. Por eso esto es una pregunta y no una
   * constante: la respuesta depende del teléfono que haya delante.
   */
  hardwareBacked: boolean
  /** Cuándo se puede leer: `kSecAttrAccessible…` en iOS, el equivalente en Android. */
  accessible: string
  detail: string
}

/**
 * Secretos pequeños, guardados donde el sistema guarda los suyos.
 *
 * En iOS es **Keychain Services** (`kSecClassGenericPassword`). En Android no
 * hay llavero —Android no tiene ninguna API que guarde secretos por ti—, así
 * que el equivalente honrado es: una clave AES en el **almacén de claves de
 * Android**, que nunca sale de él, cifrando un fichero de preferencias privado
 * de la app. Lo que protege y lo que no está en `https://angular-native.dev/extending/plugins/`, y conviene
 * leerlo antes de guardar algo aquí.
 *
 * **Esto no es una base de datos.** Son cadenas cortas: un testigo de sesión,
 * una clave de API, un PIN. Escribir un fichero entero aquí funciona y es una
 * mala idea en las dos plataformas.
 */
@Injectable({ providedIn: 'root' })
export class Keychain {
  private readonly modules = inject(NativeModules)

  /**
   * Guarda o reemplaza un secreto.
   *
   * Con `requireBiometrics` en `true` la entrada queda atada al juego de caras
   * y huellas que hay registrado ahora: si mañana se añade otra, el secreto
   * deja de poder leerse. Es lo que hace que sirva de algo.
   */
  set(key: string, value: string, options: KeychainOptions = {}): Promise<KeychainWrite> {
    return this.modules.call<KeychainWrite>('keychain', 'set', { key, value, ...options })
  }

  /** Lee un secreto. Enseña el diálogo del sistema si se guardó con biometría. */
  get(key: string, options: KeychainOptions = {}): Promise<KeychainRead> {
    return this.modules.call<KeychainRead>('keychain', 'get', { key, ...options })
  }

  /**
   * Si hay algo guardado con esa clave, sin leerlo y sin pedir biometría.
   *
   * Sirve para decidir si la pantalla enseña «desbloquear» o «configurar», que
   * es la pregunta que se hace una app al arrancar, y para la que sacar un
   * Face ID sin que nadie lo haya pedido está feo.
   */
  has(key: string): Promise<boolean> {
    return this.modules.call<boolean>('keychain', 'has', { key })
  }

  /** Borra un secreto. Dice si había algo que borrar. */
  remove(key: string): Promise<boolean> {
    return this.modules.call<boolean>('keychain', 'remove', { key })
  }

  /**
   * Qué guarda los secretos en este aparato concreto.
   *
   * No es curiosidad: en Android la respuesta cambia de un teléfono a otro, y
   * una app que guarda algo serio puede querer negarse a hacerlo en uno donde
   * el almacén es software.
   */
  backing(): Promise<KeychainBacking> {
    return this.modules.call<KeychainBacking>('keychain', 'backing')
  }
}
