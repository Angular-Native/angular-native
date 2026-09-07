import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core'
import { NATIVE_PRIMITIVES, SafeArea } from '@angular-native/primitives'
import { Biometrics } from '@angular-native/plugin-biometrics'
import { Camera } from '@angular-native/plugin-camera'
import { Clipboard } from '@angular-native/plugin-clipboard'
import { Geolocation } from '@angular-native/plugin-geolocation'
import { Keychain } from '@angular-native/plugin-keychain'
import { Preferences } from '@angular-native/plugin-preferences'

/**
 * Every plugin in the repository, in one app.
 *
 * It exists to be **compiled**: depending on all of them is what puts every
 * plugin's Swift and Java through the same `swiftc` and `javac` invocation as
 * the shell, so `scripts/check-plugin-sources.sh` finds a type error in any of
 * them without needing an app per plugin.
 *
 * It is also usable — each row calls its plugin and prints what came back,
 * refusals included. On a platform that does not have one, the sentence in the
 * row *is* the answer, and reading it is the point.
 */
interface Row {
  readonly name: string
  readonly run: () => Promise<string>
}

@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, SafeArea],
  template: `
    <an-view [style.flexGrow]="'1'" [backgroundColor]="'#0b0e14'">
      <an-safe-area [edges]="['top', 'bottom']" [padding]="20" [style.gap]="'10'">
        <an-text [fontSize]="24" [fontWeight]="'700'" [color]="'#f4f7ff'">Plugins</an-text>
        <an-text [fontSize]="13" [color]="'#8a93a6'">
          Tap one. What it prints is the plugin's own answer, refusals included.
        </an-text>

        <an-scroll-view [style.flexGrow]="'1'" [style.minHeight]="'0'"
                        [style.overflow]="'scroll'">
          <an-view [style.gap]="'10'" [style.paddingVertical]="'8'">
            @for (row of rows; track row.name) {
              <an-view
                [backgroundColor]="'#141926'"
                [borderRadius]="12"
                [style.padding]="'14'"
                [style.gap]="'6'"
                (press)="run(row)">
                <an-text [fontSize]="15" [color]="'#f4f7ff'">{{ row.name }}</an-text>
                <an-text [fontSize]="13" [color]="'#8a93a6'">
                  {{ said()[row.name] ?? 'tap to call it' }}
                </an-text>
              </an-view>
            }
          </an-view>
        </an-scroll-view>
      </an-safe-area>
    </an-view>
  `
})
export class AppComponent {
  private readonly biometrics = inject(Biometrics)
  private readonly camera = inject(Camera)
  private readonly clipboard = inject(Clipboard)
  private readonly geolocation = inject(Geolocation)
  private readonly keychain = inject(Keychain)
  private readonly preferences = inject(Preferences)

  protected readonly said = signal<Record<string, string>>({})

  protected readonly rows: readonly Row[] = [
    { name: 'preferences', run: async () => {
        await this.preferences.set('probe', 'yes')
        return `keys: ${(await this.preferences.keys()).join(', ')}`
      } },
    { name: 'clipboard', run: async () => {
        await this.clipboard.write('angular-native')
        return `read back: ${await this.clipboard.read()}`
      } },
    { name: 'keychain', run: async () => {
        const written = await this.keychain.set('probe', 'secret')
        return `${written.outcome}, on ${(await this.keychain.backing()).platform}`
      } },
    { name: 'biometrics', run: async () =>
        `availability: ${(await this.biometrics.availability()).status}` },
    { name: 'geolocation', run: async () =>
        `permission: ${await this.geolocation.permission()}` },
    { name: 'camera', run: async () =>
        `permission: ${await this.camera.permission()}` }
  ]

  protected async run(row: Row): Promise<void> {
    try {
      this.report(row.name, await row.run())
    } catch (error) {
      // The refusal is the answer on a platform that does not have the plugin,
      // so it is printed rather than swallowed.
      this.report(row.name, String(error))
    }
  }

  private report(name: string, what: string): void {
    this.said.update((all) => ({ ...all, [name]: what }))
  }
}
