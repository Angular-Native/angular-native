import { ChangeDetectionStrategy, Component, signal } from '@angular/core'
import { NATIVE_PRIMITIVES, SafeArea } from '@angular-native/primitives'
import type { NativeTextEvent } from '@angular-native/primitives'

/**
 * Navigation bar, multi-line text, embedded browser and action sheet.
 *
 * The bar and the browser belong to the system; the action sheet is presented
 * by the system and takes up no room in the layout. The `<an-web-view>` is one
 * more view: it takes whatever the layout gives it and lives among the rest.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, SafeArea],
  template: `
    <an-view [style.width]="'100%'" [style.height]="'100%'" [backgroundColor]="'#0b1020'">
      <an-safe-area [edges]="['top', 'left', 'right']" [style.flex]="1">
        <an-navigation-bar
          [style.height]="44"
          [title]="'Notes'"
          [showsBack]="true"
          [backTitle]="'Back'"
          (back)="last.set('back')" />

        <an-view [style.flex]="1" [style.padding]="'16'" [style.gap]="'12'">
          <an-text [color]="'#94a3b8'" [fontSize]="14">last: {{ last() }}</an-text>

          <an-textarea
            [style.height]="110"
            [color]="'#e2e8f0'"
            [value]="note()"
            (change)="onNote($event)"></an-textarea>

          <an-button
            [title]="'Share…'"
            [variant]="'tonal'"
            [color]="'#6ee7b7'"
            [style.height]="44"
            (press)="sheet.set(true)"></an-button>

          <an-web-view [style.flex]="1" [borderRadius]="12" [html]="page" />
        </an-view>
      </an-safe-area>

      <an-alert
        [visible]="sheet()"
        [sheet]="true"
        [title]="'Share the note'"
        [buttons]="options"
        (select)="onOption($event)" />
    </an-view>
  `
})
export class AppComponent {
  readonly options = ['Copy', 'Send by email', 'Cancel']

  readonly last = signal('nothing')
  readonly note = signal('A text of several lines.\nThe second one fits whole.')
  readonly sheet = signal(false)

  /**
   * Loose HTML instead of a URL: this way the example does not depend on there
   * being a network, and it looks the same as if it had come from outside.
   */
  readonly page = `
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <body style="margin:0;font:16px -apple-system,system-ui;background:#111a33;color:#cbd5f5">
      <div style="padding:16px">
        <h2 style="margin:0 0 8px">Inside a WebView</h2>
        <p style="margin:0;color:#94a3b8">
          This is drawn by the system browser, not by the framework.
        </p>
      </div>
    </body>`

  onNote(event: NativeTextEvent): void {
    this.note.set(event.value)
    this.last.set(`you typed ${event.value.length} characters`)
  }

  onOption(index: number): void {
    this.sheet.set(false)
    this.last.set(this.options[index] ?? 'nothing')
  }
}
