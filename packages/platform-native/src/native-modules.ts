import { inject, Injectable, NgZone } from '@angular/core'

declare const __an_native: {
  call(module: string, method: string, args: unknown): Promise<unknown>
}

/**
 * Puerta a los módulos nativos.
 *
 * Un módulo es código Rust que no pinta nada: leer el dispositivo, guardar un
 * fichero, pedir permisos. La llamada nunca bloquea; la respuesta llega en un
 * frame, el mismo o uno posterior.
 *
 * La forma recomendada de usarlo no es esta clase directamente sino un
 * servicio tipado por módulo, que es donde viven los tipos de ida y vuelta.
 */
@Injectable({ providedIn: 'root' })
export class NativeModules {
  call<T>(module: string, method: string, args?: unknown): Promise<T> {
    return __an_native.call(module, method, args) as Promise<T>
  }
}

/** Fuera de un contexto de inyección. */
export function callNative<T>(module: string, method: string, args?: unknown): Promise<T> {
  return __an_native.call(module, method, args) as Promise<T>
}

export interface DeviceInfo {
  /** `ios` o `android`. */
  platform: string
  /** Versión del sistema, tal cual la da la plataforma. */
  systemVersion: string
  /** Nombre del modelo. */
  model: string
  /** Puntos por píxel: 2 o 3 en iOS. */
  scale: number
  /** Idioma preferido del sistema, en formato BCP 47. */
  locale: string
}

/**
 * Módulo de dispositivo. Es el ejemplo de referencia de cómo se envuelve un
 * módulo nativo en un servicio tipado.
 */
@Injectable({ providedIn: 'root' })
export class Device {
  private readonly modules = inject(NativeModules)
  private readonly zone = inject(NgZone, { optional: true })

  info(): Promise<DeviceInfo> {
    void this.zone
    return this.modules.call<DeviceInfo>('device', 'info')
  }
}
