import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core'
import { Files, Share } from '@angular-native/platform'
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

      <an-view [style.gap]="'10'">
        <an-view
          [style.height]="'44'"
          [style.alignItems]="'center'"
          [style.justifyContent]="'center'"
          [backgroundColor]="'#2f6fed'"
          [borderRadius]="10"
          (press)="pick()">
          <an-text [fontSize]="15" [fontWeight]="'600'" [color]="'#ffffff'">pick a file</an-text>
        </an-view>

        <an-view
          [style.height]="'44'"
          [style.alignItems]="'center'"
          [style.justifyContent]="'center'"
          [backgroundColor]="'#1e2a4a'"
          [borderRadius]="10"
          (press)="share()">
          <an-text [fontSize]="15" [fontWeight]="'600'" [color]="'#9fb0d4'">share something</an-text>
        </an-view>
      </an-view>
    </an-view>
  `
})
export class AppComponent {
  private readonly files = inject(Files)
  private readonly sharing = inject(Share)

  readonly lines = signal<string[]>([])

  constructor() {
    void this.exercise()
  }

  private async exercise(): Promise<void> {
    await this.exerciseFiles()
    await this.exerciseShare()
  }

  /** The picker is not run on startup: it puts something on the screen. */
  pick(): void {
    void this.pickAndRead()
  }

  /**
   * Pick, then read what was picked. The second half is the interesting one: a
   * picked file is outside the app's directories, so reading it is the one thing
   * the module allows there, and only because it came back from `pick()`.
   */
  private async pickAndRead(): Promise<void> {
    try {
      const picked = await this.files.pick({ multiple: false })
      if (picked.length === 0) {
        // Somebody closed the picker. Not a failure, and not a rejection.
        this.say('files.pick: nothing was chosen')
        return
      }
      const contents = await this.files.read(picked[0].path)
      this.say(`files.pick: ok ${picked[0].name}, read ${contents.length} bytes back`)
    } catch (error) {
      this.say(`files.pick: no ${error}`)
    }
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
  /**
   * The sheet is not opened on startup either. What is checked without anybody
   * present is the one thing that can be: whether this platform has a sheet at
   * all, which is a question three of the seven answer `false`.
   */
  private async exerciseShare(): Promise<void> {
    try {
      const can = await this.sharing.canShare()
      this.say(`share.canShare: ok ${can ? 'yes' : 'no, not on this platform'}`)
    } catch (error) {
      this.say(`share.canShare: no ${error}`)
    }
  }

  /** Opens the system sheet. Cancelling is not a failure and does not throw. */
  share(): void {
    this.sharing
      .share({ title: 'angular-native', text: 'shared from a built-in module' })
      .then((done) => this.say(`share.share: ok ${done ? 'shared' : 'cancelled'}`))
      .catch((error: unknown) => this.say(`share.share: no ${error}`))
  }

  private say(line: string): void {
    this.lines.update((lines) => [...lines, line])
    console.log(`[modules] ${line}`)
  }
}
