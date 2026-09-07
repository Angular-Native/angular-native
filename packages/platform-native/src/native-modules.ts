import { inject, Injectable, NgZone } from '@angular/core'

declare const __an_native: {
  call(module: string, method: string, args: unknown): Promise<unknown>
  on(module: string, event: string, handler: (payload: unknown) => void): () => void
}

/**
 * The door to the native modules.
 *
 * A module is Rust code that draws nothing: reading the device, saving a file,
 * asking for permissions. The call never blocks; the answer arrives in a frame,
 * this one or a later one.
 *
 * The recommended way to use it is not this class directly but a typed service
 * per module, which is where the types going out and coming back live.
 */
@Injectable({ providedIn: 'root' })
export class NativeModules {
  call<T>(module: string, method: string, args?: unknown): Promise<T> {
    return __an_native.call(module, method, args) as Promise<T>
  }

  /**
   * Listens to what a module says without being asked.
   *
   * A call has exactly one answer, which is why `call` returns a promise. A
   * position while walking, a notification being tapped, a socket's messages:
   * those have none or a thousand, and a promise cannot carry them. This is the
   * road for those, and it is the reason a module can be a source rather than
   * only a service.
   *
   * Events arrive at the top of a frame, in the order they were emitted, and
   * after that frame's answers — so a `start()` that resolves and immediately
   * emits settles before its first event lands.
   *
   * ```ts
   * const stop = modules.on<Position>('geolocation', 'position', (where) => …)
   * inject(DestroyRef).onDestroy(stop)
   * ```
   *
   * @returns the unsubscriber. Calling it twice is harmless. **A subscription
   * that is never stopped keeps the handler, and whatever it closes over,
   * alive for the life of the app.**
   */
  on<T>(module: string, event: string, handler: (payload: T) => void): () => void {
    return __an_native.on(module, event, handler as (payload: unknown) => void)
  }
}

/** Outside an injection context. */
export function callNative<T>(module: string, method: string, args?: unknown): Promise<T> {
  return __an_native.call(module, method, args) as Promise<T>
}

/** The same, for listening outside an injection context. */
export function onNative<T>(
  module: string,
  event: string,
  handler: (payload: T) => void
): () => void {
  return __an_native.on(module, event, handler as (payload: unknown) => void)
}

/**
 * Where the app is running.
 *
 * One per host, not one per operating system: `ios` is the iPad too, because
 * they are the same host and the same surface; `tvos`, `visionos`, `macos`,
 * `watchos` and `wearos` really are different places, with different controls
 * and different ways of being handled, and an app that wants to adapt needs to
 * tell them apart.
 */
export type NativePlatform =
  | 'ios'
  | 'tvos'
  | 'visionos'
  | 'macos'
  | 'watchos'
  | 'android'
  | 'wearos'

export interface DeviceInfo {
  /** Where it is running. See `NativePlatform`. */
  platform: NativePlatform
  /** The system's version, exactly as the platform gives it. */
  systemVersion: string
  /** The model's name. */
  model: string
  /** Points per pixel: 2 or 3 on iOS. */
  scale: number
  /** The system's preferred language, in BCP 47 form. */
  locale: string
}

/**
 * The device module. It is the reference example of how a native module gets
 * wrapped in a typed service.
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
