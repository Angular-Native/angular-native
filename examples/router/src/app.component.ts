import { ChangeDetectionStrategy, Component } from '@angular/core'
import { RouterOutlet } from '@angular/router'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'

/**
 * El `<router-outlet>` no tiene contrapartida nativa: se monta como `View`,
 * igual que el host de cualquier componente, y el router mete y saca páginas
 * dentro.
 *
 * Todavía no hay navegación nativa: no hay `UINavigationController` ni gesto
 * de volver atrás. La ruta cambia y la vista se sustituye, sin animación.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, RouterOutlet],
  template: `
    <View
      [style.width]="'100%'"
      [style.height]="'100%'"
      [style.paddingTop]="'64'"
      [backgroundColor]="'#0b1020'">
      <router-outlet />
    </View>
  `
})
export class AppComponent {}
