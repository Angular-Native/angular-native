import { ChangeDetectionStrategy, Component } from '@angular/core'
import { NativeStack } from '@angular-native/platform'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'

/**
 * `NativeStack` va donde iría un `<router-outlet>`: la pantalla nueva entra
 * deslizándose, la anterior se queda viva por debajo, y el gesto de borde de
 * iOS y el botón de atrás de Android navegan hacia atrás.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, NativeStack],
  template: `
    <View
      [style.width]="'100%'"
      [style.height]="'100%'"
      [style.paddingTop]="'64'"
      [backgroundColor]="'#0b1020'">
      <NativeStack />
    </View>
  `
})
export class AppComponent {}
