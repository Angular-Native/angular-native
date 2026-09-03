import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core'
import {
  Biometrics,
  type BiometricAvailability,
  type BiometricOutcome
} from '@angular-native/plugin-biometrics'
import { Keychain, type KeychainReadOutcome } from '@angular-native/plugin-keychain'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'

/** The key it is stored under. A real app would have more than one. */
const KEY = 'session-token'

/** What is stored. Fake here; in an app it would be the session token. */
const SECRET = 'sk_live_9f3a-nobody-should-see-this'

/**
 * Every ending an authentication can have, said for whoever is in front of it.
 *
 * It is the reason `authenticate` does not return a boolean: the eleven
 * situations call for eleven sentences, and four of them also call for the app
 * to do something different —send them to Settings, offer the passcode, hide the
 * button—. With a `true`/`false` this table could not be written.
 */
const EXPLANATION: Record<BiometricOutcome, string> = {
  success: 'authenticated',
  failed: 'it did not recognise you',
  userCancel: 'you cancelled it yourself',
  userFallback: 'you asked for the passcode; try again allowing it',
  systemCancel: 'the system closed it',
  timeout: 'it timed out',
  noHardware: 'this device has no biometric sensor',
  notEnrolled: 'there is no face or fingerprint enrolled: go to Settings',
  passcodeNotSet: 'the device has no passcode, and without one there is no biometrics',
  lockedOut: 'too many attempts; unlock the device with the passcode',
  permanentlyLockedOut: 'locked out until you unlock the device with the passcode',
  unavailable: 'the system is not lending out biometrics right now'
}

const READING: Record<KeychainReadOutcome, string> = {
  found: 'read',
  notFound: 'there is nothing stored yet',
  denied: 'it is stored and you have not proved it is you',
  invalidated: 'the device biometrics changed: it can no longer be opened',
  unavailable: 'there is nowhere to store this on this device'
}

/**
 * A secret kept in the system keychain and protected with biometrics.
 *
 * The two plugins go together on purpose: storing something "securely" that
 * anybody can read back protects nothing, and asking for a face without tying it
 * to what does the encrypting is theatre. What the button at the bottom does is
 * the second one done properly: the secret is stored with an access control that
 * demands biometrics, and what unlocks it is the system, not this code.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-view
      [style.width]="'100%'"
      [style.height]="'100%'"
      [style.paddingTop]="'72'"
      [style.paddingHorizontal]="'20'"
      [style.gap]="'14'"
      [backgroundColor]="'#0b1020'">

      <an-text [fontSize]="28" [fontWeight]="'bold'" [color]="'#f4f7ff'">secrets</an-text>

      <an-text [fontSize]="14" [color]="'#9fb0d4'">{{ sensor() }}</an-text>

      <an-view
        [style.paddingVertical]="'14'"
        [style.paddingHorizontal]="'14'"
        [style.minHeight]="'56'"
        [style.justifyContent]="'center'"
        [backgroundColor]="'#1e2a4a'"
        [borderRadius]="10">
        <an-text [fontSize]="16" [color]="'#f4f7ff'">{{ contents() }}</an-text>
      </an-view>

      <an-text [fontSize]="13" [color]="'#9fb0d4'">{{ store() }}</an-text>

      <an-view [style.flexDirection]="'row'" [style.gap]="'10'">
        <an-view
          [style.flexGrow]="'1'"
          [style.height]="'46'"
          [style.alignItems]="'center'"
          [style.justifyContent]="'center'"
          [backgroundColor]="'#2f6fed'"
          [borderRadius]="10"
          (press)="save()">
          <an-text [fontSize]="15" [fontWeight]="'600'" [color]="'#ffffff'">save</an-text>
        </an-view>

        <an-view
          [style.flexGrow]="'1'"
          [style.height]="'46'"
          [style.alignItems]="'center'"
          [style.justifyContent]="'center'"
          [backgroundColor]="'#1e2a4a'"
          [borderRadius]="10"
          (press)="read()">
          <an-text [fontSize]="15" [fontWeight]="'600'" [color]="'#9fb0d4'">read</an-text>
        </an-view>

        <an-view
          [style.flexGrow]="'1'"
          [style.height]="'46'"
          [style.alignItems]="'center'"
          [style.justifyContent]="'center'"
          [backgroundColor]="'#1e2a4a'"
          [borderRadius]="10"
          (press)="remove()">
          <an-text [fontSize]="15" [fontWeight]="'600'" [color]="'#9fb0d4'">delete</an-text>
        </an-view>
      </an-view>

      <an-view
        [style.height]="'46'"
        [style.alignItems]="'center'"
        [style.justifyContent]="'center'"
        [backgroundColor]="'#1e2a4a'"
        [borderRadius]="10"
        (press)="check()">
        <an-text [fontSize]="15" [fontWeight]="'600'" [color]="'#9fb0d4'">
          just authenticate, read nothing
        </an-text>
      </an-view>

      <an-text [fontSize]="15" [color]="'#6ee7b7'">{{ status() }}</an-text>
      <an-text [fontSize]="12" [color]="'#7d8bb0'">{{ detail() }}</an-text>
    </an-view>
  `
})
export class AppComponent {
  private readonly biometrics = inject(Biometrics)
  private readonly keychain = inject(Keychain)

  readonly sensor = signal('asking the system…')
  /** Whether anything is stored. It is kept apart from `status` on purpose: it
   * is what stays true after the last action ends and its message goes by. */
  readonly store = signal('asking the keychain…')
  readonly contents = signal('—')
  readonly status = signal('')
  readonly detail = signal('')

  constructor() {
    // On startup it asks what is there, without bringing up any dialog:
    // `availability` interrupts nobody. What the button says comes from here.
    this.biometrics
      .availability()
      .then((info) => this.sensor.set(this.describe(info)))
      .catch((error: unknown) => this.failed(error))

    // And if there is already something stored, it says so —without reading it—.
    // `has` does not ask for the face either: knowing the item exists is not
    // opening it.
    this.keychain
      .has(KEY)
      .then((there) => this.store.set(there ? 'there is a secret stored' : 'there is nothing stored'))
      .catch((error: unknown) => this.failed(error))
  }

  /**
   * Stores the secret demanding biometrics to read it back.
   *
   * On iOS this asks nothing: the dialog comes up on reading. On Android it does
   * ask, because there the key that encrypts is single-use and authenticated.
   * The difference is in the plugin's contract, not hidden away in here.
   */
  save(): void {
    this.keychain
      .set(KEY, SECRET, {
        requireBiometrics: true,
        reason: 'Store the session token protected with your face'
      })
      .then((result) => {
        this.detail.set(result.detail)
        if (result.outcome === 'saved') {
          this.contents.set('—')
          this.store.set('there is a secret stored')
          this.status.set('stored; your face is now needed to read it')
          return
        }
        this.status.set(
          result.outcome === 'denied'
            ? 'not stored: you did not authenticate'
            : 'it cannot be stored on this device'
        )
      })
      .catch((error: unknown) => this.failed(error))
  }

  /** Reads the secret. This is where iOS brings up Face ID. */
  read(): void {
    this.status.set('asking for your face…')
    this.keychain
      .get(KEY, { reason: 'Show the stored session token' })
      .then((reading) => {
        this.detail.set(reading.detail)
        this.status.set(READING[reading.outcome])
        // A value is only shown on `found`. On `denied` the secret is still
        // stored and unopened, which is not the same as not having it.
        this.contents.set(reading.outcome === 'found' ? (reading.value ?? '') : '—')
      })
      .catch((error: unknown) => this.failed(error))
  }

  remove(): void {
    this.keychain
      .remove(KEY)
      .then((there) => {
        this.contents.set('—')
        this.store.set('there is nothing stored')
        this.detail.set('')
        this.status.set(there ? 'deleted' : 'there was nothing to delete')
      })
      .catch((error: unknown) => this.failed(error))
  }

  /** Biometrics on its own, with no keychain in the middle. */
  check(): void {
    this.status.set('asking for your face…')
    this.biometrics
      .authenticate({
        reason: 'Check that it is you',
        subtitle: 'reading no secrets',
        cancelTitle: 'Not now'
      })
      .then((result) => {
        this.status.set(EXPLANATION[result.outcome])
        this.detail.set(`${result.kind} · ${result.detail}`)
      })
      .catch((error: unknown) => this.failed(error))
  }

  private describe(info: BiometricAvailability): string {
    const names: Record<string, string> = {
      faceId: 'Face ID',
      touchId: 'Touch ID',
      opticId: 'Optic ID',
      unknown: 'biometrics',
      none: 'no sensor'
    }
    const what = names[info.kind] ?? info.kind
    return info.status === 'available'
      ? `${what}, ready`
      : `${what}: ${EXPLANATION[info.status]}`
  }

  /**
   * A missing plugin is not glossed over. Without the plugin inside the `.app`
   * the promise rejects, and that is exactly what has to be seen.
   */
  private failed(error: unknown): void {
    this.contents.set('—')
    this.status.set(`failed: ${error}`)
  }
}
