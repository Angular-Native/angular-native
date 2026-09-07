import { inject, Injectable } from '@angular/core'
import { NativeModules } from '@angular-native/platform'

/**
 * Small values that outlive the app being closed.
 *
 * `UserDefaults` on the Apple platforms and `SharedPreferences` on Android: the
 * store every one of them already has for settings, which means it is backed up
 * with the device, migrated by the system between OS versions, and readable
 * before the first frame.
 *
 * **Strings only, on purpose.** Both stores can hold numbers, booleans, dates
 * and arrays, and the sets they can hold are not the same set. A key written as
 * a number on one platform and read as a string on the other is a bug that only
 * shows up on the platform nobody tested, so the wire carries one type and
 * anything structured goes through `JSON.stringify`. It is the same decision the
 * native-module boundary already makes.
 *
 * **This is not secure storage.** A preferences file is plain text on a device
 * somebody can root. Tokens, passwords and keys belong in
 * `@angular-native/plugin-keychain`, which is backed by the Secure Enclave and
 * the Android keystore.
 *
 * ```ts
 * const prefs = inject(Preferences)
 * await prefs.set('theme', 'dark')
 * const theme = await prefs.get('theme')      // 'dark', or null
 * ```
 */
@Injectable({ providedIn: 'root' })
export class Preferences {
  private readonly modules = inject(NativeModules)

  /** What is stored under that key, or `null` if nothing is. */
  get(key: string): Promise<string | null> {
    return this.modules.call<string | null>('preferences', 'get', { key })
  }

  /** Writes it. An existing value under the same key is replaced. */
  set(key: string, value: string): Promise<void> {
    return this.modules.call<void>('preferences', 'set', { key, value })
  }

  /** Removes it. Removing a key that is not there is not an error. */
  remove(key: string): Promise<void> {
    return this.modules.call<void>('preferences', 'remove', { key })
  }

  /**
   * Every key this app has written, in no particular order.
   *
   * It never returns the platform's own keys: the store is a suite of its own on
   * Apple and a named file on Android, so what comes back is only what went in
   * through here.
   */
  keys(): Promise<string[]> {
    return this.modules.call<string[]>('preferences', 'keys')
  }

  /** Empties the store. Only what this plugin wrote. */
  clear(): Promise<void> {
    return this.modules.call<void>('preferences', 'clear')
  }
}
