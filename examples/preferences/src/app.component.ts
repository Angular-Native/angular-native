import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core'
import { NATIVE_PRIMITIVES, SafeArea } from '@angular-native/primitives'
import { Preferences } from '@angular-native/plugin-preferences'

/**
 * The preferences plugin, end to end.
 *
 * Every line the app prints is the plugin's own answer, including the failures:
 * what is being checked is not that the calls resolve but that what went in on
 * one launch comes back out on the next.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, SafeArea],
  template: `
    <an-view [style.flexGrow]="'1'" [backgroundColor]="'#0b0e14'">
      <an-safe-area [edges]="['top', 'bottom']" [padding]="20" [style.gap]="'14'">
        <an-text [fontSize]="24" [fontWeight]="'700'" [color]="'#f4f7ff'">Preferences</an-text>
        <an-text [fontSize]="13" [color]="'#8a93a6'">
          Write something, close the app from the switcher, and open it again.
        </an-text>

        <an-text-input
          [placeholder]="'A note that survives a restart'"
          [value]="draft()"
          [style.height]="'40'"
          (valueChange)="draft.set($event)" />

        <an-view [style.flexDirection]="'row'" [style.gap]="'10'">
          <an-button [title]="'Save'" [variant]="'filled'" [style.flex]="'1'"
                     [style.height]="'42'" (press)="save()" />
          <an-button [title]="'Reload'" [style.flex]="'1'" [style.height]="'42'"
                     (press)="load()" />
          <an-button [title]="'Clear'" [style.flex]="'1'" [style.height]="'42'"
                     (press)="wipe()" />
        </an-view>

        <an-text [fontSize]="13" [color]="'#8a93a6'">stored</an-text>
        <an-text [fontSize]="16" [color]="'#f4f7ff'">{{ stored() ?? '(nothing yet)' }}</an-text>

        <an-text [fontSize]="13" [color]="'#8a93a6'">keys</an-text>
        <an-text [fontSize]="14" [color]="'#c8d2e8'">{{ keys().join(', ') || '(none)' }}</an-text>

        <an-text [fontSize]="13" [color]="'#ff5a1f'">{{ problem() }}</an-text>
      </an-safe-area>
    </an-view>
  `
})
export class AppComponent {
  private readonly preferences = inject(Preferences)

  protected readonly draft = signal('')
  protected readonly stored = signal<string | null>(null)
  protected readonly keys = signal<string[]>([])
  protected readonly problem = signal('')

  constructor() {
    void this.load()
  }

  protected async save(): Promise<void> {
    await this.guard(async () => {
      await this.preferences.set('note', this.draft())
      await this.refresh()
    })
  }

  protected async load(): Promise<void> {
    await this.guard(() => this.refresh())
  }

  protected async wipe(): Promise<void> {
    await this.guard(async () => {
      await this.preferences.clear()
      await this.refresh()
    })
  }

  private async refresh(): Promise<void> {
    const note = await this.preferences.get('note')
    this.stored.set(note)
    this.draft.set(note ?? '')
    this.keys.set(await this.preferences.keys())
  }

  /** A rejection is printed rather than swallowed: on a platform without the
   *  plugin, the reason it gives is the whole point of the example. */
  private async guard(work: () => Promise<void>): Promise<void> {
    try {
      this.problem.set('')
      await work()
    } catch (error) {
      this.problem.set(String(error))
    }
  }
}
