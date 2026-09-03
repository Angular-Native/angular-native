import { inject, Injectable } from '@angular/core'
import { NativeModules } from '@angular-native/platform'

/**
 * Which biometry the device has.
 *
 * iOS says so: `LAContext.biometryType` tells Touch ID, Face ID and Optic ID
 * apart. Android does not —`BiometricManager` answers whether it can
 * authenticate, not with what—, so over there whatever is available arrives as
 * `unknown`. That is the honest answer: making up `'fingerprint'` because most
 * Androids are would be lying about exactly the ones with a camera.
 */
export type BiometryKind = 'faceId' | 'touchId' | 'opticId' | 'unknown' | 'none'

/**
 * Whether biometrics can be asked for right now, and if not, why not.
 *
 * The four reasons are not the same one: with no sensor there is nothing to
 * offer and the button is pointless; with nothing enrolled the button is worth
 * having but it has to send the user to Settings; locked out by attempts is
 * fixed with the device passcode; and locked out for good is not fixed at all
 * short of unlocking the phone. An app that treats them alike ends up showing
 * "it did not work" over all four.
 */
export type BiometricStatus =
  /** There is a sensor, something is enrolled, and the system would allow it now. */
  | 'available'
  /** The device has no biometric sensor. */
  | 'noHardware'
  /** There is a sensor, but nobody has enrolled a face or a fingerprint. */
  | 'notEnrolled'
  /** The device has no passcode, and without one there is no biometry worth anything. */
  | 'passcodeNotSet'
  /** Too many failed attempts. It recovers by unlocking with the passcode. */
  | 'lockedOut'
  /**
   * Locked out until the device is unlocked with the passcode. Only Android
   * tells this apart from the previous one; iOS folds both into `lockedOut`.
   */
  | 'permanentlyLockedOut'
  /**
   * There is a sensor but the system is not lending it right now: busy, waiting
   * on a security update, or a version of Android older than the one that
   * brought `BiometricPrompt`. `detail` says which.
   */
  | 'unavailable'

export interface BiometricAvailability {
  status: BiometricStatus
  kind: BiometryKind
  /**
   * What the system said, verbatim: the name of the `LAError` or the
   * `BiometricManager` constant.
   *
   * **It is not text to show the user.** It is in English, it changes between
   * system versions and it explains nothing to anyone who did not write the
   * app. It is for the log and for a bug report.
   */
  detail: string
}

/**
 * How an authentication ended.
 *
 * A boolean would flatten the eleven into two, and the eleven call for
 * different answers: `userCancel` does not deserve so much as a warning —the
 * user knows they cancelled—, `notEnrolled` deserves a link to Settings,
 * `lockedOut` deserves offering the passcode, and `failed` deserves another go.
 */
export type BiometricOutcome =
  /** Authenticated. It is the only one that means yes. */
  | 'success'
  /**
   * Biometry ran, recognised nobody, and the system called the session over. On
   * iOS it arrives after several attempts in a row without a match; on Android
   * a single failure ends nothing —the dialog stays on screen— and what ends up
   * arriving is `lockedOut`.
   */
  | 'failed'
  /** The user dismissed the dialog or pressed the cancel button. */
  | 'userCancel'
  /**
   * The user asked for the device passcode and falling back to it had not been
   * allowed (`allowDeviceCredential` set to `false`). iOS only. Whoever wants to
   * handle it can call again with `allowDeviceCredential: true`.
   */
  | 'userFallback'
  /** The system took the dialog away: the app went to the background, a call came in. */
  | 'systemCancel'
  /**
   * Time ran out staring at the sensor.
   *
   * It only ever arrives from Android, which has `BIOMETRIC_ERROR_TIMEOUT`. iOS
   * times out too, but the `LAError` it returns —the −1003— is not in the public
   * enum, so it arrives as `unavailable` with
   * `"LAError -1003: Authentication timed out."` inside `detail`. Translating a
   * number Apple does not document would be guessing; the message is exact.
   */
  | 'timeout'
  | 'noHardware'
  | 'notEnrolled'
  | 'passcodeNotSet'
  | 'lockedOut'
  | 'permanentlyLockedOut'
  | 'unavailable'

export interface BiometricResult {
  outcome: BiometricOutcome
  kind: BiometryKind
  /** Like {@link BiometricAvailability.detail}: diagnostic, not interface. */
  detail: string
}

export interface BiometricRequest {
  /**
   * Why it is being asked for. iOS shows it under the Face ID icon; Android, as
   * the title of the dialog. Mandatory on both: a system dialog that does not
   * say what it is for is one nobody gets past.
   */
  reason: string
  /** Second line of the dialog. Only Android has room for it. */
  subtitle?: string
  /** The text of the cancel button. By default, the system's own. */
  cancelTitle?: string
  /**
   * Let the user in with the device passcode, pattern or password if biometrics
   * do not work out.
   *
   * It changes what is being checked: with this set to `true`, `success` means
   * "whoever is holding the device knows how to open it", not "this is the
   * enrolled face". For unlocking a screen it is usually fine; for authorising a
   * payment, it is not.
   */
  allowDeviceCredential?: boolean
}

/**
 * The system's biometrics: `LAContext` on iOS and `BiometricPrompt` on Android.
 *
 * The dialog is drawn by the system, not by the app: nothing is painted here and
 * the fingerprint or the face is never seen. The only thing that crosses the
 * boundary is how it ended.
 *
 * Neither of the two methods rejects the promise over something that can happen
 * to a user. Cancelling, failing, having no sensor and being locked out are
 * answers, not errors, and they arrive in `outcome`. The promise is only
 * rejected when the mistake is the programmer's: a method that does not exist, a
 * missing `reason`, or the plugin not compiled into the app.
 */
@Injectable({ providedIn: 'root' })
export class Biometrics {
  private readonly modules = inject(NativeModules)

  /**
   * Whether biometrics can be asked for, without asking for them: it shows no
   * dialog.
   *
   * It is there to decide whether the button appears and what it says. What it
   * is **not** for is taking the result for granted: between this call and the
   * next one the user may have gone off to Settings and deleted their
   * fingerprint, so `authenticate` tells the whole story again.
   */
  availability(): Promise<BiometricAvailability> {
    return this.modules.call<BiometricAvailability>('biometrics', 'availability')
  }

  /** Shows the system dialog and tells how it ended. */
  authenticate(request: BiometricRequest): Promise<BiometricResult> {
    return this.modules.call<BiometricResult>('biometrics', 'authenticate', request)
  }
}
