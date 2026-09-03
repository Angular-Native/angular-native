import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core'
import { Clipboard } from '@angular-native/plugin-clipboard'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'

const PHRASES = [
  'hello from angular-native',
  'a plugin is an npm package',
  'Swift and Java, compiled from source'
]

/**
 * The system clipboard, from a plugin.
 *
 * The app does not know there is a plugin in the middle: it injects `Clipboard`
 * and calls its methods, just as it would inject `Device`. The only thing that
 * tells it apart from any other app is the line in `package.json` that declares
 * the dependency; everything else follows from there, including `an` compiling
 * the Swift and the Java.
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
      [style.gap]="'16'"
      [backgroundColor]="'#0b1020'">

      <an-text [fontSize]="28" [fontWeight]="'bold'" [color]="'#f4f7ff'">clipboard</an-text>

      <an-text [fontSize]="14" [color]="'#9fb0d4'">
        one plugin: an npm package, Swift, Java and this line of TypeScript
      </an-text>

      <an-view
        [style.paddingVertical]="'14'"
        [style.paddingHorizontal]="'14'"
        [backgroundColor]="'#1e2a4a'"
        [borderRadius]="10">
        <an-text [fontSize]="17" [color]="'#f4f7ff'">{{ text() }}</an-text>
      </an-view>

      <an-view [style.flexDirection]="'row'" [style.gap]="'12'">
        <an-view
          [style.flexGrow]="'1'"
          [style.height]="'46'"
          [style.alignItems]="'center'"
          [style.justifyContent]="'center'"
          [backgroundColor]="'#2f6fed'"
          [borderRadius]="10"
          (press)="copy()">
          <an-text [fontSize]="16" [fontWeight]="'600'" [color]="'#ffffff'">copy</an-text>
        </an-view>

        <an-view
          [style.flexGrow]="'1'"
          [style.height]="'46'"
          [style.alignItems]="'center'"
          [style.justifyContent]="'center'"
          [backgroundColor]="'#1e2a4a'"
          [borderRadius]="10"
          (press)="paste()">
          <an-text [fontSize]="16" [fontWeight]="'600'" [color]="'#9fb0d4'">paste</an-text>
        </an-view>

        <an-view
          [style.flexGrow]="'1'"
          [style.height]="'46'"
          [style.alignItems]="'center'"
          [style.justifyContent]="'center'"
          [backgroundColor]="'#1e2a4a'"
          [borderRadius]="10"
          (press)="next()">
          <an-text [fontSize]="16" [fontWeight]="'600'" [color]="'#9fb0d4'">another phrase</an-text>
        </an-view>
      </an-view>

      <an-text [fontSize]="15" [color]="'#6ee7b7'">on the clipboard: {{ contents() }}</an-text>
      <an-text [fontSize]="13" [color]="'#f2b8b5'">{{ status() }}</an-text>
    </an-view>
  `
})
export class AppComponent {
  private readonly clipboard = inject(Clipboard)

  private readonly phrase = signal(0)
  readonly text = computed(() => PHRASES[this.phrase() % PHRASES.length])
  readonly contents = signal('…')
  readonly status = signal('')

  constructor() {
    // On startup it asks whether there is anything, not what there is: on iOS 16
    // and later, reading what another app copied shows a system banner, and
    // throwing one at somebody the moment they open the app without their
    // having asked for anything is rude. `hasText` does not trigger it. The call
    // does not block: the answer arrives in a frame, the same one or a later one.
    this.clipboard
      .hasText()
      .then((there) => this.status.set(there ? 'something is copied' : 'the clipboard is empty'))
      .catch((error: unknown) => this.failed(error))
  }

  copy(): void {
    this.clipboard
      .write(this.text())
      .then(() => {
        this.status.set('copied')
        // Re-reading what the app itself has just written does not trigger the banner.
        return this.paste()
      })
      .catch((error: unknown) => this.failed(error))
  }

  paste(): void {
    this.clipboard
      .read()
      .then((text) => this.contents.set(text === '' ? '(empty)' : text))
      .catch((error: unknown) => this.failed(error))
  }

  next(): void {
    this.phrase.update((current) => current + 1)
  }

  /**
   * A missing plugin is not glossed over: the reason is shown. With no plugin in
   * the `.app` the promise rejects, and that is exactly what has to happen.
   */
  private failed(error: unknown): void {
    this.contents.set('—')
    this.status.set(`the clipboard failed: ${error}`)
  }
}
