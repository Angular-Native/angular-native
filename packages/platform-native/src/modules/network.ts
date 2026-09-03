import { inject, Injectable, signal, type Signal } from '@angular/core'

import { NativeModules, type NativePlatform } from '../native-modules'

/**
 * Which interface the way out goes through.
 *
 * `'none'` is not an interface called none: it is what there is when there is
 * no path at all. Reporting that as `'other'` would be reporting a connection
 * where there is none.
 */
export type NetworkConnection = 'wifi' | 'cellular' | 'ethernet' | 'other' | 'none'

export interface NetworkStatus {
  /** Whether there is a way out at all. */
  online: boolean
  connection: NetworkConnection
  /** Metered: mobile data, or somebody's hotspot. Ask before downloading something large. */
  expensive: boolean
  /** The person asked for less traffic: Low Data Mode on Apple's platforms, Data Saver on Android. */
  constrained: boolean
}

/**
 * Where the network monitor exists: everywhere.
 *
 * It is the only one of the four built-ins with no exceptions. A watch, a
 * television and a headset all have a network, `NWPathMonitor` exists on all
 * four Apple platforms and `ConnectivityManager` on both Android ones. The type
 * is written out anyway, because the other three have exclusions and a reader
 * comparing them should be able to see that this one does not.
 */
export type NetworkPlatform = NativePlatform

/**
 * What the system believes about the way out.
 *
 * `NWPathMonitor` on Apple's platforms, `ConnectivityManager.NetworkCallback` on
 * Android. Neither is a reachability check against some address and neither is a
 * request that failed: both are the platform's own view, and both are already
 * running before the first call, which is why `read()` answers at once.
 *
 * On Android it needs `ACCESS_NETWORK_STATE` in the manifest. It is an
 * install-time permission, so there is nothing to ask the person: the shell
 * declares it, and an app whose manifest lacks it gets a rejection with the line
 * to paste instead of a `SecurityException`.
 */
@Injectable({ providedIn: 'root' })
export class Network {
  private readonly modules = inject(NativeModules)
  private readonly latest = signal<NetworkStatus | null>(null)
  private timer: ReturnType<typeof setInterval> | null = null
  private watchers = 0

  /**
   * The last thing the monitor said, or `null` before anything has been asked.
   *
   * It is only kept up to date while something is watching — see `watch()`. On
   * its own it is the memory of the last `read()`.
   */
  readonly status: Signal<NetworkStatus | null> = this.latest.asReadonly()

  /** Asks the monitor and updates `status`. */
  async read(): Promise<NetworkStatus> {
    const status = await this.modules.call<NetworkStatus>('network', 'status')
    this.latest.set(status)
    return status
  }

  /**
   * Keeps `status` fresh, and returns the function that stops it.
   *
   * The monitor underneath is event-driven and always right: it is the system
   * pushing at the moment the Wi-Fi drops. What is missing is the last hop —
   * there is no channel from a native module back into JS yet, only answers to
   * calls — so this asks it on an interval. The polling is ours and the
   * monitoring is the platform's, and that is said here rather than dressed up
   * as a subscription. When that channel exists, this method keeps its shape and
   * loses the timer.
   */
  watch(everyMs = 2000): () => void {
    this.watchers += 1
    void this.read()
    this.timer ??= setInterval(() => {
      void this.read()
    }, everyMs)
    let stopped = false
    return () => {
      if (stopped) return
      stopped = true
      this.watchers -= 1
      if (this.watchers === 0 && this.timer !== null) {
        clearInterval(this.timer)
        this.timer = null
      }
    }
  }
}
