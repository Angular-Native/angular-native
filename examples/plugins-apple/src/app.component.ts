import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core'
import { NATIVE_PRIMITIVES, SafeArea } from '@angular-native/primitives'
import { BarcodeScanner } from '@angular-native/plugin-barcode'

/**
 * The plugins that only exist on Apple platforms.
 *
 * They are here and not in `examples/plugins` for a reason worth knowing: an app
 * that depends on a plugin with no Android half **cannot be built for Android**,
 * and the build says so instead of shipping an app whose calls would all be
 * rejected. So the app that exercises every plugin on three platforms cannot be
 * the same app that exercises this one.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, SafeArea],
  template: `
    <an-view [style.flexGrow]="'1'" [backgroundColor]="'#0b0e14'">
      <an-safe-area [edges]="['top', 'bottom']" [padding]="20" [style.gap]="'12'">
        <an-text [fontSize]="24" [fontWeight]="'700'" [color]="'#f4f7ff'">Barcode</an-text>
        <an-text [fontSize]="13" [color]="'#8a93a6'">
          The scanner opens full screen. There is no inline preview: a preview is
          a view, and a plugin contributes methods.
        </an-text>
        <an-button [title]="'Scan'" [variant]="'filled'" [style.height]="'44'"
                   (press)="scan()" />
        <an-text [fontSize]="15" [color]="'#f4f7ff'">{{ said() }}</an-text>
      </an-safe-area>
    </an-view>
  `
})
export class AppComponent {
  private readonly barcode = inject(BarcodeScanner)
  protected readonly said = signal('nothing read yet')

  protected async scan(): Promise<void> {
    try {
      const found = await this.barcode.scan()
      this.said.set(`${found.format}: ${found.value}`)
    } catch (error) {
      // Cancelling rejects and says so, which is the whole point of the message.
      this.said.set(String(error))
    }
  }
}
