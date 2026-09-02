import { ChangeDetectionStrategy, Component, signal } from '@angular/core'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'

/**
 * La misma clase de componente que corre en el teléfono, en una ventana
 * volumétrica.
 *
 * Dos cosas que esta pantalla enseña a propósito:
 *
 * 1. **El fondo no se pinta entero.** La ventana de visionOS ya trae uno: el
 *    cristal que dibuja el sistema, con su desenfoque y su sombra sobre la
 *    habitación de verdad. El shell deja la raíz transparente y aquí solo se
 *    pintan las tarjetas, así que el cristal se ve entre ellas. Un
 *    `[backgroundColor]` en el contenedor de arriba lo taparía y la app sería
 *    una losa opaca flotando en el salón.
 *
 * 2. **No hay tamaño de pantalla.** El usuario tira de la esquina y la ventana
 *    cambia de tamaño cuando quiere. Nada de aquí está en puntos fijos: los
 *    anchos van en porcentaje y en `flexGrow`, y el viewport llega por
 *    `viewDidLayoutSubviews` como en cualquier otra familia. Los `an-view` con
 *    `(press)` los realza el sistema al mirarlos, porque el host les pone
 *    `hoverStyle`.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-view
      [style.width]="'100%'"
      [style.height]="'100%'"
      [style.padding]="'40'"
      [style.gap]="'24'">

      <an-text [fontSize]="44" [fontWeight]="'bold'" [color]="'#f4f7ff'">angular-native</an-text>

      <an-text [fontSize]="20" [color]="'#c8d3ea'">
        Sin fondo propio: lo que se ve detrás es el cristal de la ventana, que
        lo pinta el sistema. Mira una tarjeta y pellízcala.
      </an-text>

      <an-view [style.flexDirection]="'row'" [style.gap]="'24'" [style.width]="'100%'">
        @for (tarjeta of tarjetas; track tarjeta) {
          <an-view
            [style.flexGrow]="'1'"
            [style.height]="'160'"
            [borderRadius]="24"
            [backgroundColor]="'#1e2a4a'"
            (press)="elige(tarjeta)">
            <an-text
              [style.width]="'100%'"
              [style.height]="'160'"
              [fontSize]="24"
              [textAlign]="'center'"
              [color]="'#f4f7ff'">{{ tarjeta }}</an-text>
          </an-view>
        }
      </an-view>

      <an-text [fontSize]="20" [color]="'#9fb0d4'">{{ elegida() }}</an-text>
    </an-view>
  `
})
export class AppComponent {
  readonly tarjetas = ['una', 'dos', 'tres']
  readonly elegida = signal('nada elegido todavía')

  elige(tarjeta: string): void {
    this.elegida.set(`elegiste: ${tarjeta}`)
  }
}
