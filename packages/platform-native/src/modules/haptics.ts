import { inject, Injectable } from '@angular/core'

import { NativeModules, type NativePlatform } from '../native-modules'

/**
 * How hard the tap is.
 *
 * The five are UIKit's, because they are the only set with five distinct
 * feelings behind them. Elsewhere they are mapped onto what the hardware has:
 * three predefined effects on Android, one click on the watch, two patterns on a
 * Force Touch trackpad. A platform never pretends to more resolution than it
 * has.
 */
export type HapticImpact = 'light' | 'medium' | 'heavy' | 'soft' | 'rigid'

/** What happened, rather than how it feels. */
export type HapticNotification = 'success' | 'warning' | 'error'

/** What this device can actually do. Ask before relying on any of it. */
export interface HapticSupport {
  /** Whether `impact()` and `selection()` reach anything at all. */
  available: boolean
  /** Whether the three notification patterns exist. `false` on macOS. */
  notification: boolean
  /**
   * Empty when there is nothing to warn about; otherwise why what is asked for
   * may not be felt. It is a sentence for a developer, not for a person.
   */
  caveat: string
}

/**
 * Where haptics exist.
 *
 * Not on a television: there is nothing to vibrate and the Siri Remote has no
 * engine an app can drive. Not in the headset: nothing is held and nothing
 * touches the wrist. Both reject by name and both answer `available: false` from
 * `support()`, so an app can decide before it asks.
 *
 * macOS is in the type and only half in the hardware — the feedback goes to a
 * Force Touch trackpad if the Mac has one, and AppKit cannot say whether it
 * does. That is why `support()` carries a `caveat` and not just a boolean: it is
 * the one platform where the honest answer is a sentence.
 */
export type HapticsPlatform = Exclude<NativePlatform, 'tvos' | 'visionos'>

/**
 * The tap the device gives back.
 *
 * `UIImpactFeedbackGenerator` and friends on iOS and iPadOS,
 * `WKInterfaceDevice.play` on the watch, `VibrationEffect` on Android,
 * `NSHapticFeedbackManager` on a Mac with a Force Touch trackpad. Every one of
 * them is the platform's own, and the differences between them are hardware
 * rather than API: a watch has one click and no weights, and saying otherwise
 * would be the app being told it got what it asked for.
 *
 * On Android it needs `VIBRATE` in the manifest, which is install-time: the
 * shell declares it, and an app whose manifest lacks it gets a rejection with
 * the line to paste.
 */
@Injectable({ providedIn: 'root' })
export class Haptics {
  private readonly modules = inject(NativeModules)

  /** What this device can do, and what it cannot. */
  support(): Promise<HapticSupport> {
    return this.modules.call<HapticSupport>('haptics', 'support')
  }

  /** Something happened. The weight is a hint; see `HapticImpact`. */
  impact(style: HapticImpact = 'medium'): Promise<void> {
    return this.modules.call<void>('haptics', 'impact', { style })
  }

  /**
   * Something finished, well or badly.
   *
   * Rejects on macOS, where there is no pattern that means any of the three and
   * playing one feeling for all three would lose the distinction.
   */
  notification(type: HapticNotification): Promise<void> {
    return this.modules.call<void>('haptics', 'notification', { type })
  }

  /** The tick a value makes as it changes: a picker turning, a segment moving. */
  selection(): Promise<void> {
    return this.modules.call<void>('haptics', 'selection')
  }
}
