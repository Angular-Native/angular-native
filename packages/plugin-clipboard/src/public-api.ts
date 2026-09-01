import { inject, Injectable } from '@angular/core'
import { NativeModules } from '@angular-native/platform'

/**
 * El portapapeles del sistema.
 *
 * Es el ejemplo de referencia de cómo se escribe un plugin: un servicio de
 * Angular que no tiene lógica ninguna —los tipos de ida y vuelta y poco más—
 * sobre un módulo nativo que sí la tiene, y que está escrito una vez en Swift
 * y otra en Java.
 *
 * En iOS es `UIPasteboard`; en Android, `ClipboardManager`. Ninguna de las dos
 * se puede tocar desde el hilo del motor, así que las llamadas van por la cola
 * de plugins y se atienden en el hilo de UI. De ahí que todo devuelva promesa.
 */
@Injectable({ providedIn: 'root' })
export class Clipboard {
  private readonly modules = inject(NativeModules)

  /** Copia un texto. */
  write(text: string): Promise<void> {
    return this.modules.call<void>('clipboard', 'write', { text })
  }

  /**
   * Lo que haya copiado, o cadena vacía si no hay texto.
   *
   * En iOS 16 y posteriores, leer algo que copió otra app enseña un aviso del
   * sistema; leer lo que copió la propia app, no.
   */
  read(): Promise<string> {
    return this.modules.call<string>('clipboard', 'read')
  }

  /** Si hay texto, sin leerlo. En iOS esto no dispara el aviso del sistema. */
  hasText(): Promise<boolean> {
    return this.modules.call<boolean>('clipboard', 'hasText')
  }
}
