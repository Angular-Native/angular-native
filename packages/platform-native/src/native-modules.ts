import { inject, Injectable, NgZone } from '@angular/core'

declare const __an_native: {
  call(module: string, method: string, args: unknown): Promise<unknown>
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
}

/** Outside an injection context. */
export function callNative<T>(module: string, method: string, args?: unknown): Promise<T> {
  return __an_native.call(module, method, args) as Promise<T>
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
