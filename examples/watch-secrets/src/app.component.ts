import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core'
import { Keychain, type KeychainReadOutcome } from '@angular-native/plugin-keychain'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'

/** The key it is stored under. A real app would have more than one. */
const KEY = 'watch-token'

/** What is stored. Fake here; in an app it would be the session token. */
const SECRET = 'wk_live_watch-only'

const READING: Record<KeychainReadOutcome, string> = {
  found: 'read back',
  notFound: 'nothing stored yet',
  denied: 'the watch is locked',
  invalidated: 'the item can no longer be opened',
  unavailable: 'there is nowhere to store this'
}

/**
 * A secret in the watch's keychain.
 *
 * This is `examples/secrets` minus the half a watch cannot run, and the
 * subtraction is the whole point of the example. That one depends on
 * `plugin-biometrics` as well, and `an watchos` refuses to build it — a watch
 * has no sensor, the plugin says so in its `package.json`, and the refusal
 * quotes it. Keychain Services, on the other hand, is all there: the same
 * `SecItemAdd`, the same Secure Enclave underneath, the same `.app`-private
 * store.
 *
 * What is missing here compared with the phone is one option, and it is missing
 * out loud rather than quietly downgraded: `requireBiometrics` comes back
 * `unavailable` with nothing stored. Pressing "behind biometrics" shows exactly
 * that, which is the reason the button is there.
 *
 * The measurements are a watch's. On a screen 176 points wide the phone's
 * `fontSize` of 28 eats half the view.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <!--
      The height goes through \`flexGrow\` and not \`height: 100%\`: the core gives
      everything scrollable \`flex-basis: 0\` so it fits inside its parent, and on
      the main axis the basis beats the height.
    -->
    <an-scroll-view [style.width]="'100%'" [style.flexGrow]="'1'" [backgroundColor]="'#0b1020'">
      <an-view
        [style.paddingTop]="'44'"
        [style.paddingHorizontal]="'10'"
        [style.paddingBottom]="'16'"
        [style.gap]="'8'"
        [style.width]="'100%'">

        <an-text [fontSize]="18" [fontWeight]="'bold'" [color]="'#f4f7ff'">keychain</an-text>

        <!--
          The round trip's verdict, right under the title and above everything
          else, because on a watch that is the only line a check can read. See
          roundTrip() below.
        -->
        <an-text [fontSize]="13" [color]="'#6ee7b7'">{{ roundTripResult() }}</an-text>

        <an-text [fontSize]="12" [color]="'#9fb0d4'">{{ store() }}</an-text>

        <an-view
          [style.paddingVertical]="'8'"
          [style.paddingHorizontal]="'8'"
          [style.width]="'100%'"
          [backgroundColor]="'#1e2a4a'"
          [borderRadius]="8">
          <an-text [fontSize]="13" [color]="'#f4f7ff'">{{ contents() }}</an-text>
        </an-view>

        <an-button [title]="'save'" (press)="save()"></an-button>
        <an-button [title]="'read'" (press)="read()"></an-button>
        <an-button [title]="'delete'" (press)="remove()"></an-button>
        <an-button [title]="'behind biometrics'" (press)="saveProtected()"></an-button>

        <an-text [fontSize]="13" [color]="'#6ee7b7'">{{ status() }}</an-text>
        <an-text [fontSize]="11" [color]="'#7d8bb0'">{{ detail() }}</an-text>
      </an-view>
    </an-scroll-view>
  `
})
export class AppComponent {
  private readonly keychain = inject(Keychain)

  readonly store = signal('asking the keychain…')
  readonly contents = signal('—')
  readonly status = signal('')
  readonly detail = signal('')
  readonly roundTripResult = signal('storing and reading back…')

  constructor() {
    void this.roundTrip()
  }

  /**
   * The verdict, on screen and in the log.
   *
   * On screen for a person, in the log for a check: `simctl io … screenshot` can
   * take the picture but nothing reads a sentence out of it, and `simctl launch
   * --console-pty` is the one channel that comes back as text from a watch
   * simulator without borrowing the Mac's mouse.
   */
  private verdict(said: string): void {
    this.roundTripResult.set(said)
    console.log(said)
  }

  /**
   * Store, read back, delete — on startup, with nobody pressing anything.
   *
   * The buttons below do the same three things one at a time and they are what a
   * person would use. This exists for the other reader: a check, which on a
   * watch simulator can take a screenshot and very little else. Tapping the
   * screen means moving the Mac's own mouse over the Simulator window (see
   * `scripts/watch-input.sh`), which needs a desktop session that is unlocked
   * and in front — and that is exactly what a check running on somebody else's
   * machine, or over ssh, does not have.
   *
   * So the whole round trip runs by itself and writes one line saying how it
   * went. If the plugin never reached the watch, the promise rejects and the
   * line says that instead; either way the screenshot answers the question.
   */
  private async roundTrip(): Promise<void> {
    try {
      const backing = await this.keychain.backing()
      this.detail.set(`${backing.platform} · hardware: ${backing.hardwareBacked}`)

      // From a clean slate: a leftover from the previous run would make a read
      // succeed without this run's write having worked.
      await this.keychain.remove(KEY)

      const stored = await this.keychain.set(KEY, SECRET)
      if (stored.outcome !== 'saved') {
        this.verdict(`round trip: not stored (${stored.outcome})`)
        this.detail.set(stored.detail)
        return
      }

      const read = await this.keychain.get(KEY)
      if (read.outcome !== 'found' || read.value !== SECRET) {
        this.verdict(`round trip: read back ${read.outcome}`)
        this.detail.set(read.detail)
        return
      }

      await this.keychain.remove(KEY)
      const left = await this.keychain.has(KEY)
      this.store.set(left ? 'a secret is stored' : 'nothing stored')
      if (left) {
        this.verdict('round trip: stored and read, but delete left it there')
        return
      }

      // And the gap, checked as carefully as the part that works. A watch has no
      // biometric sensor, so this has to come back refused with nothing stored.
      // If it ever came back `saved`, some future version would have quietly
      // substituted the passcode for the fingerprint, and the only place that
      // would show is right here.
      const protectedSave = await this.keychain.set(KEY, SECRET, {
        requireBiometrics: true,
        reason: 'Show the stored token'
      })
      const refused = protectedSave.outcome !== 'saved' && !(await this.keychain.has(KEY))
      this.verdict(
        refused
          ? 'round trip: stored, read and deleted; biometrics refused'
          : `round trip: biometrics was NOT refused (${protectedSave.outcome})`
      )
    } catch (error: unknown) {
      // The one that matters most. With no plugin in the `.app` every call
      // rejects, and a screen that just sat there saying nothing would be the
      // silent failure this whole system exists to avoid.
      this.verdict(`round trip failed: ${error}`)
    }
  }

  /**
   * Stores it with no biometrics, which on a watch is the only way there is.
   *
   * It is not an unprotected store: the item is
   * `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`, and on a watch that
   * window is much shorter than the same constant means on a phone left on a
   * desk — the watch locks itself the moment it comes off the wrist.
   */
  save(): void {
    this.keychain
      .set(KEY, SECRET)
      .then((result) => {
        this.detail.set(result.detail)
        if (result.outcome === 'saved') {
          this.contents.set('—')
          this.store.set('a secret is stored')
          this.status.set('stored')
          return
        }
        this.status.set(`not stored: ${result.outcome}`)
      })
      .catch((error: unknown) => this.failed(error))
  }

  /**
   * The refusal, on purpose and in the open.
   *
   * A watch has no biometric sensor. This could have stored the item behind the
   * watch's passcode and answered `saved`, and that is the one thing it must not
   * do: an app told its secret is behind a fingerprint when it is behind a
   * four-digit code has been lied to about the only thing it asked.
   */
  saveProtected(): void {
    this.keychain
      .set(KEY, SECRET, { requireBiometrics: true, reason: 'Show the stored token' })
      .then((result) => {
        this.detail.set(result.detail)
        this.status.set(result.outcome === 'saved' ? 'stored' : `refused: ${result.outcome}`)
      })
      .catch((error: unknown) => this.failed(error))
  }

  read(): void {
    this.keychain
      .get(KEY)
      .then((reading) => {
        this.detail.set(reading.detail)
        this.status.set(READING[reading.outcome])
        this.contents.set(reading.outcome === 'found' ? (reading.value ?? '') : '—')
      })
      .catch((error: unknown) => this.failed(error))
  }

  remove(): void {
    this.keychain
      .remove(KEY)
      .then((there) => {
        this.contents.set('—')
        this.store.set('nothing stored')
        this.status.set(there ? 'deleted' : 'there was nothing to delete')
      })
      .catch((error: unknown) => this.failed(error))
  }

  /**
   * A missing plugin is not glossed over. Without the plugin inside the `.app`
   * the promise rejects, and that is exactly what has to be seen — on a watch it
   * is also the likeliest thing to go wrong, because a plugin that works on the
   * phone is not here unless it declared a watchOS half.
   */
  private failed(error: unknown): void {
    this.contents.set('—')
    this.status.set(`failed: ${error}`)
  }
}
