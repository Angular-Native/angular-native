import { DestroyRef, inject, Injectable } from '@angular/core'
import { NativeModules } from '@angular-native/platform'

/** Where the device is, as the system reports it. */
export interface Position {
  latitude: number
  longitude: number
  /** Metres. The radius the system is confident about, not a guess of ours. */
  accuracy: number
  /** Metres above sea level, or `null` where the fix has no altitude. */
  altitude: number | null
  /** Metres per second, or `null` when the device is not moving or cannot tell. */
  speed: number | null
  /** Degrees clockwise from true north, or `null`. */
  heading: number | null
  /** Milliseconds since the epoch, from the fix and not from the clock now. */
  timestamp: number
}

/** What the person has decided about this app and their location. */
export type LocationPermission = 'granted' | 'denied' | 'prompt' | 'restricted'

/**
 * The device's location.
 *
 * `CLLocationManager` on the Apple platforms and `LocationManager` on Android —
 * the platform's own, not Google's: Play Services is not on every Android device
 * and pulling it in would make an app that cannot be installed on the ones
 * without it.
 *
 * **A location is asked for, never assumed.** `current()` starts by checking the
 * permission and rejects with what the person actually chose, so an app can say
 * "location is off in Settings" rather than spinning for ever. Both platforms
 * also require a reason string before the dialog will even appear — see the
 * permissions section of the plugin's page.
 *
 * ```ts
 * const geo = inject(Geolocation)
 * if (await geo.request() === 'granted') {
 *   const here = await geo.current()
 * }
 * ```
 */
@Injectable({ providedIn: 'root' })
export class Geolocation {
  private readonly modules = inject(NativeModules)

  /** What the person has already decided, without asking them again. */
  permission(): Promise<LocationPermission> {
    return this.modules.call<LocationPermission>('geolocation', 'permission')
  }

  /**
   * Asks, if there is anything to ask.
   *
   * Already answered means no dialog and the standing answer comes straight
   * back: the system only shows the prompt once, and asking again from an app is
   * how you get an app that nags.
   */
  request(): Promise<LocationPermission> {
    return this.modules.call<LocationPermission>('geolocation', 'request')
  }

  /**
   * One fix.
   *
   * It rejects rather than resolving with nothing: no permission, location
   * switched off on the device, or nothing arriving before the timeout are three
   * different sentences, and an app that cannot tell them apart cannot say
   * anything useful to the person.
   *
   * @param timeoutMs how long to wait for a fix. 10 seconds by default.
   */
  current(timeoutMs = 10_000): Promise<Position> {
    return this.modules.call<Position>('geolocation', 'current', { timeoutMs })
  }

  /**
   * Every fix, until you stop.
   *
   * This is not `current()` in a loop. The platform decides when there is a new
   * position worth reporting — it knows the radio is already awake, it knows the
   * device has not moved — and a poll would either miss movements or keep the
   * GPS on for nothing. Fixes arrive through the module event channel, at the
   * top of a frame, in the order the system produced them.
   *
   * ```ts
   * const stop = geo.watch((where) => this.here.set(where))
   * ```
   *
   * The returned function stops the stream and releases the platform's listener.
   * **Call it.** A watch nobody stops keeps the location hardware running, which
   * on a phone is measured in percentage points of battery per hour.
   *
   * In an injection context it is stopped for you when the component is
   * destroyed; outside one, that is yours to do.
   *
   * @param onFix called with each new position.
   * @param minMetres how far the device has to move before the next fix. Zero
   *   means every fix the platform produces.
   */
  watch(onFix: (position: Position) => void, minMetres = 0): () => void {
    const off = this.modules.on<Position>('geolocation', 'position', onFix)
    let stopped = false
    const stop = () => {
      if (stopped) return
      stopped = true
      off()
      // The platform is told last: a fix already in this frame's mailbox should
      // still reach a handler that has not been detached yet.
      void this.modules.call<void>('geolocation', 'unwatch').catch(() => {
        // Stopping a watch that is already stopped is not worth a rejection
        // reaching the app: the caller asked for it to be off, and it is off.
      })
    }
    void this.modules.call<void>('geolocation', 'watch', { minMetres }).catch((error) => {
      stop()
      throw error
    })
    inject(DestroyRef, { optional: true })?.onDestroy(stop)
    return stop
  }
}
