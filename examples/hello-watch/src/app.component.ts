import { ChangeDetectionStrategy, Component, computed, signal } from '@angular/core'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'

/**
 * La misma clase de componente que corre en el teléfono, con el subconjunto de
 * primitivas que el reloj sabe pintar hoy: `View`, `Text`, `Button` y
 * `ScrollView`.
 *
 * Las medidas son de reloj y no de teléfono. En una pantalla de 176 puntos de
 * ancho un `fontSize` de 28 se come la mitad de la vista, así que la cabecera
 * es 18 y el cuerpo 13. El `paddingTop` tampoco es el del iPhone: aquí no hay
 * muesca, pero sí el reloj del sistema pintado encima de la app en la esquina
 * de arriba, y hay que dejarle sitio.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <!--
      El alto va por \`flexGrow\` y no por \`height: 100%\`: el núcleo le pone a todo
      lo scrollable \`flex-basis: 0\` para que quepa en su padre, y en el eje
      principal la base gana al alto. Con \`height\` a secas el ScrollView mide
      cero y no se ve nada.
    -->
    <an-scroll-view [style.width]="'100%'" [style.flexGrow]="'1'" [backgroundColor]="'#0b1020'">
      <an-view
        [style.paddingTop]="'44'"
        [style.paddingHorizontal]="'10'"
        [style.paddingBottom]="'16'"
        [style.gap]="'8'"
        [style.width]="'100%'">

        <an-text [fontSize]="18" [fontWeight]="'bold'" [color]="'#f4f7ff'">angular-native</an-text>

        <an-text [fontSize]="13" [color]="'#9fb0d4'">
          Angular con señales, en QuickJS, con el layout de taffy. Aquí lo pinta
          SwiftUI porque el reloj no tiene UIView.
        </an-text>

        <an-view
          [style.height]="'44'"
          [style.width]="'100%'"
          [borderRadius]="10"
          [backgroundColor]="'#1e2a4a'"
          (press)="toca()">
          <an-text
            [style.width]="'100%'"
            [style.height]="'44'"
            [fontSize]="15"
            [textAlign]="'center'"
            [color]="'#6ee7b7'">{{ etiqueta() }}</an-text>
        </an-view>

        <an-button
          [title]="'poner a cero'"
          [color]="'#f59e0b'"
          [backgroundColor]="'#2b1e4a'"
          [borderRadius]="10"
          (press)="toques.set(0)"></an-button>

        <!-- Para que haya algo que desplazar y se vea que la corona funciona. -->
        @for (fila of filas; track fila) {
          <an-view [style.height]="'26'" [borderRadius]="6" [backgroundColor]="'#152036'">
            <an-text
              [style.width]="'100%'"
              [style.height]="'26'"
              [fontSize]="12"
              [textAlign]="'center'"
              [color]="'#9fb0d4'">{{ fila }}</an-text>
          </an-view>
        }
      </an-view>
    </an-scroll-view>
  `
})
export class AppComponent {
  readonly toques = signal(0)
  readonly segundos = signal(0)

  readonly etiqueta = computed(() =>
    this.toques() === 0 ? 'toca aquí' : `toques: ${this.toques()}`
  )

  readonly filas = ['una', 'dos', 'tres', 'cuatro', 'cinco', 'seis']

  toca(): void {
    this.toques.update((valor) => valor + 1)
  }

  constructor() {
    // El reloj de JS lo marca el frame, no un hilo aparte: esto avanza con el
    // temporizador del shell, igual que en iOS avanza con el CADisplayLink.
    setInterval(() => this.segundos.update((valor) => valor + 1), 1000)
  }
}
