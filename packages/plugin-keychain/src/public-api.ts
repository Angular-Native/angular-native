import { inject, Injectable } from '@angular/core'
import { NativeModules } from '@angular-native/platform'

/**
 * How a read ended.
 *
 * `notFound` and `denied` are not the same thing, and flattening them into "no
 * value" is the classic mistake in this kind of API: the first one means the
 * password has to be asked for again, and the second one means the secret is
 * still there and whoever is holding the device has not proved they own it.
 * Logging the user out in the second case would be punishing them for having
 * dismissed a dialog.
 */
export type KeychainReadOutcome =
  /** It was there, and here it is. */
  | 'found'
  /** Nothing is stored under that key. */
  | 'notFound'
  /**
   * It is stored and it could not be opened: the user cancelled the biometric
   * check, did not pass it, or is locked out by attempts. The value is still
   * where it was.
   */
  | 'denied'
  /**
   * It was stored and it can never be decrypted again: whoever holds the device
   * has changed the enrolled fingerprints or faces, and an entry protected by
   * biometrics is bound to the set that was there when it was saved.
   *
   * Only Android tells this apart. iOS deletes the item when that happens, so
   * over there the same situation arrives as `notFound`.
   */
  | 'invalidated'
  /** There is nowhere to put this: no sensor, no device passcode, no store. */
  | 'unavailable'

export interface KeychainRead {
  outcome: KeychainReadOutcome
  /** The secret, and only with `found`. In any other case it is `null`. */
  value: string | null
  /** What the system said. Diagnostic, not text to show. */
  detail: string
}

export type KeychainWriteOutcome = 'saved' | 'denied' | 'unavailable'

export interface KeychainWrite {
  outcome: KeychainWriteOutcome
  detail: string
}

export interface KeychainOptions {
  /**
   * Demand biometrics —or the device passcode— to read it back.
   *
   * **The two platforms do not ask for the same thing at the same moment.** On
   * iOS saving asks nothing and reading shows Face ID or Touch ID. On Android
   * the key that encrypts is single-use and authenticated, so **saving asks for
   * the fingerprint too**. It is not an oversight: it is how it is: the Android
   * key store ties authentication to every single use of the key, be it
   * encrypting or decrypting. Whoever does not want that difference should not
   * use this option.
   */
  requireBiometrics?: boolean
  /**
   * What the system dialog shows. Mandatory if `requireBiometrics` is `true`: a
   * dialog that does not say what it is for is one nobody gets past.
   */
  reason?: string
}

/** What really stores the secrets on this device. */
export interface KeychainBacking {
  platform: 'ios' | 'android'
  /**
   * Whether the key lives in hardware the system itself cannot look into: the
   * Secure Enclave on iOS, the TEE or the StrongBox on Android.
   *
   * On Android there are devices that do not have it and the key store is
   * software. There is still encryption there and the key still never leaves the
   * system, but an attacker with root can walk off with it. That is why this is
   * a question and not a constant: the answer depends on which phone is in
   * front of you.
   */
  hardwareBacked: boolean
  /** When it can be read: `kSecAttrAccessible…` on iOS, the equivalent on Android. */
  accessible: string
  detail: string
}

/**
 * Small secrets, stored where the system stores its own.
 *
 * On iOS that is **Keychain Services** (`kSecClassGenericPassword`). On Android
 * there is no keychain —Android has no API that stores secrets for you—, so the
 * honest equivalent is: an AES key in the **Android key store**, which never
 * leaves it, encrypting a preferences file private to the app. What it protects
 * and what it does not is at `https://angular-native.dev/extending/plugins/`, and it is
 * worth reading before storing anything here.
 *
 * **This is not a database.** These are short strings: a session token, an API
 * key, a PIN. Writing a whole file in here works and is a bad idea on both
 * platforms.
 */
@Injectable({ providedIn: 'root' })
export class Keychain {
  private readonly modules = inject(NativeModules)

  /**
   * Stores or replaces a secret.
   *
   * With `requireBiometrics` set to `true` the entry is bound to the set of
   * faces and fingerprints enrolled right now: if another one is added
   * tomorrow, the secret can no longer be read. That is what makes it worth
   * anything.
   */
  set(key: string, value: string, options: KeychainOptions = {}): Promise<KeychainWrite> {
    return this.modules.call<KeychainWrite>('keychain', 'set', { key, value, ...options })
  }

  /** Reads a secret. Shows the system dialog if it was stored with biometrics. */
  get(key: string, options: KeychainOptions = {}): Promise<KeychainRead> {
    return this.modules.call<KeychainRead>('keychain', 'get', { key, ...options })
  }

  /**
   * Whether anything is stored under that key, without reading it and without
   * asking for biometrics.
   *
   * It is there to decide whether the screen shows "unlock" or "set up", which
   * is the question an app asks itself on startup, and one for which throwing a
   * Face ID at the user nobody asked for looks bad.
   */
  has(key: string): Promise<boolean> {
    return this.modules.call<boolean>('keychain', 'has', { key })
  }

  /** Deletes a secret. Says whether there was anything to delete. */
  remove(key: string): Promise<boolean> {
    return this.modules.call<boolean>('keychain', 'remove', { key })
  }

  /**
   * What stores the secrets on this particular device.
   *
   * It is not curiosity: on Android the answer changes from one phone to
   * another, and an app storing something serious may want to refuse to do it
   * on one where the store is software.
   */
  backing(): Promise<KeychainBacking> {
    return this.modules.call<KeychainBacking>('keychain', 'backing')
  }
}
