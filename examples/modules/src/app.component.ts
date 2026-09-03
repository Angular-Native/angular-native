import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core'
import { Files } from '@angular-native/platform'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'

/**
 * The framework's built-in modules, exercised one line at a time.
 *
 * Every line is written as `<module>: ok …` or `<module>: no …`, and neither of
 * the two is a mistake: a platform that cannot do something says so and the app
 * shows what it said. `scripts/check-builtins.sh` reads these very lines out of
 * the running macOS app, which is why the shape of them matters.
 *
 * Nothing here declares a dependency. That is the whole point of a module being
 * built in: `Files` is injected the way `Device` is, and there is no npm package
 * and no `angularNative` block anywhere.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-view
      [style.width]="'100%'"
      [style.height]="'100%'"
      [style.paddingTop]="'56'"
      [style.paddingHorizontal]="'20'"
      [style.gap]="'10'"
      [backgroundColor]="'#0b1020'">

      <an-text [fontSize]="26" [fontWeight]="'bold'" [color]="'#f4f7ff'">built-in modules</an-text>

      <an-text [fontSize]="13" [color]="'#9fb0d4'">
        four modules every host has, with no plugin and no dependency
      </an-text>

      @for (line of lines(); track line) {
        <an-text [fontSize]="14" [color]="'#c7d5f5'">{{ line }}</an-text>
      }

      <an-view
        [style.height]="'44'"
        [style.alignItems]="'center'"
        [style.justifyContent]="'center'"
        [backgroundColor]="'#2f6fed'"
        [borderRadius]="10"
        (press)="pick()">
        <an-text [fontSize]="15" [fontWeight]="'600'" [color]="'#ffffff'">pick a file</an-text>
      </an-view>
    </an-view>
  `
})
export class AppComponent {
  private readonly files = inject(Files)

  readonly lines = signal<string[]>([])

  constructor() {
    void this.exerciseFiles()
  }

  /** The picker is not run on startup: it puts something on the screen. */
  pick(): void {
    this.files
      .pick({ multiple: false })
      .then((picked) => {
        this.say(
          picked.length === 0
            ? 'files.pick: nothing was chosen'
            : `files.pick: ok ${picked[0].name} (${picked[0].size} bytes)`
        )
      })
      .catch((error: unknown) => this.say(`files.pick: no ${error}`))
  }

  /**
   * Write, read back, list, delete. It is the whole round trip through the app's
   * own directory, which is the half of the module that needs nobody's
   * permission and exists on every platform but the television.
   */
  private async exerciseFiles(): Promise<void> {
    try {
      const home = await this.files.documentsDirectory()
      this.say(`files.documentsDirectory: ok ${home}`)
    } catch (error) {
      // Which is what tvOS answers, and it is not a failure of the app: the
      // platform has no storage that lasts and says so.
      this.say(`files.documentsDirectory: no ${error}`)
    }

    try {
      await this.files.write('an-check/note.txt', 'written by the modules example')
      const back = await this.files.read('an-check/note.txt')
      const entries = await this.files.list('an-check')
      await this.files.remove('an-check')
      const gone = await this.files.exists('an-check/note.txt')
      this.say(
        `files: ok wrote and read back ${back.length} bytes, ` +
          `listed ${entries.length}, deleted ${gone ? 'nothing' : 'it'}`
      )
    } catch (error) {
      this.say(`files: no ${error}`)
    }

    try {
      await this.files.read('/etc/passwd')
      this.say('files.sandbox: no it read a file outside the app')
    } catch {
      // The rule the module enforces: outside its own directories it reads only
      // what the person picked. A rejection here is the check passing.
      this.say('files.sandbox: ok a path outside the app is refused')
    }
  }

  /**
   * On the screen and in the log, both on purpose. The screen is what a person
   * sees; the log is what a check reads, and on Android it is the only one it
   * can read without a working `uiautomator`.
   */
  private say(line: string): void {
    this.lines.update((lines) => [...lines, line])
    console.log(`[modules] ${line}`)
  }
}
