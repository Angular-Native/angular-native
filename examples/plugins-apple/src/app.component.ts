import { ChangeDetectionStrategy, Component, DestroyRef, inject, signal } from '@angular/core'
import { NATIVE_PRIMITIVES, SafeArea } from '@angular-native/primitives'
import { NativeModules } from '@angular-native/platform'
import { BarcodeScanner } from '@angular-native/plugin-barcode'

/**
 * The plugins that only exist on Apple platforms — and the one thing a plugin
 * could not do until now.
 *
 * The preview below is a **view a plugin brought**. Everything else a plugin
 * contributes is a method; this is mounted with `<an-custom>` and laid out like
 * any other box, and what it reads arrives on the module event channel rather
 * than as the answer to a call.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, SafeArea],
  template: `
    <an-view [style.flexGrow]="'1'" [backgroundColor]="'#0b0e14'">
      <an-safe-area [edges]="['top', 'bottom']" [padding]="20" [style.gap]="'14'">
        <an-text [fontSize]="24" [fontWeight]="'700'" [color]="'#f4f7ff'">Barcode</an-text>

        <an-text [fontSize]="13" [color]="'#8a93a6'">
          A live preview, inside the layout. It is a plugin's view mounted through
          an-custom, sized by flexbox like anything else.
        </an-text>

        <an-custom
          [view]="'barcode-preview'"
          [style.height]="'260'"
          [borderRadius]="16"
          [backgroundColor]="'#141926'" />

        <an-text [fontSize]="15" [color]="'#f4f7ff'">{{ read() }}</an-text>

        <an-text [fontSize]="13" [color]="'#8a93a6'">
          And the same reader full screen, which is what a plugin could always do:
        </an-text>
        <an-button [title]="'Scan full screen'" [variant]="'filled'"
                   [style.height]="'44'" (press)="scan()" />
      </an-safe-area>
    </an-view>
  `
})
export class AppComponent {
  private readonly barcode = inject(BarcodeScanner)
  private readonly modules = inject(NativeModules)

  protected readonly read = signal('nothing read yet')

  constructor() {
    // The preview emits; it does not answer. A view that reads a barcode
    // produces none or a hundred, and a promise carries one.
    const stop = this.modules.on<{ value: string; format: string }>(
      'barcode',
      'read',
      (found) => this.read.set(`${found.format}: ${found.value}`)
    )
    inject(DestroyRef).onDestroy(stop)
  }

  protected async scan(): Promise<void> {
    try {
      const found = await this.barcode.scan()
      this.read.set(`${found.format}: ${found.value}`)
    } catch (error) {
      this.read.set(String(error))
    }
  }
}
