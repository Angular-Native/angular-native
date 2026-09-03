import { ChangeDetectionStrategy, Component } from '@angular/core'
import { NativeStack } from '@angular-native/platform'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'

/**
 * `NativeStack` goes where a `<router-outlet>` would go: the new screen slides
 * in, the previous one stays alive underneath, and the iOS edge gesture and the
 * Android back button navigate back.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, NativeStack],
  template: `
    <an-view
      [style.width]="'100%'"
      [style.height]="'100%'"
      [style.paddingTop]="'64'"
      [backgroundColor]="'#0b1020'">
      <an-native-stack />
    </an-view>
  `
})
export class AppComponent {}
