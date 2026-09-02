import { ChangeDetectionStrategy, Component, signal } from '@angular/core'
import { NATIVE_PRIMITIVES, SafeArea } from '@angular-native/primitives'

/**
 * Wear OS con las primitivas de siempre.
 *
 * A diferencia del reloj de Apple, aquí no hay nada de reloj en el código: un
 * Wear OS corre `android.view.View`, así que el host es el mismo que el del
 * teléfono y estas etiquetas son las mismas. Lo que cambia es la pantalla, y
 * cambia en dos cosas que sí se ven desde la plantilla:
 *
 * 1. **Es redonda.** Lo que se ponga en la esquina no se recorta: no se pinta,
 *    porque ahí no hay pantalla. Quien lo aparta es `an-safe-area`, igual que
 *    aparta del notch en el teléfono; el host cuenta el cuadrado inscrito en la
 *    circunferencia como un margen más del sistema. La plantilla no sabe si el
 *    margen viene de una muesca o de una curva, y no tiene por qué.
 *
 * 2. **Se recorre con la corona.** Eso no se declara: `an-scroll-view` la
 *    escucha en el reloj, y `(scroll)` sale igual que si el dedo la hubiera
 *    arrastrado.
 *
 * Las medidas son de esfera. La pantalla del emulador son 227 puntos, y el
 * cuadrado que cabe dentro son 160: un `fontSize` de 28 no entra.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, SafeArea],
  template: `
    <!--
      El área segura envuelve al desplazable y no al revés. Al revés el
      contenido llegaría al borde, que es lo bonito en un reloj, pero la
      primera y la última fila se comerían el arco: en una esfera, lo que está
      arriba del todo está en el punto más estrecho.
    -->
    <an-view
      [style.width]="'100%'"
      [style.flexGrow]="'1'"
      [backgroundColor]="'#0b1020'">
      <an-safe-area [style.width]="'100%'">
        <an-scroll-view
        [style.width]="'100%'"
        [style.flexGrow]="'1'"
        (scroll)="alto.set(Math.round($event.y))">
        <an-view [style.width]="'100%'" [style.gap]="'6'" [style.paddingBottom]="'8'">
          <an-text [fontSize]="16" [fontWeight]="'bold'" [color]="'#f4f7ff'">
            angular-native
          </an-text>

          <an-text [fontSize]="11" [color]="'#9fb0d4'">
            Gira la corona: {{ alto() }} pt.
          </an-text>

          @for (fila of filas; track fila.numero) {
            <an-view
              [style.width]="'100%'"
              [style.height]="'30'"
              [borderRadius]="8"
              [backgroundColor]="fila.numero === marcada() ? '#2b1e4a' : '#152036'"
              (press)="marcada.set(fila.numero)">
              <an-text
                [style.width]="'100%'"
                [style.height]="'30'"
                [fontSize]="12"
                [textAlign]="'center'"
                [color]="fila.numero === marcada() ? '#f59e0b' : '#9fb0d4'">
                {{ fila.texto }}
              </an-text>
            </an-view>
          }
        </an-view>
        </an-scroll-view>
      </an-safe-area>
    </an-view>
  `
})
export class AppComponent {
  /** La plantilla no ve los globales; el redondeo se hace con este. */
  protected readonly Math = Math

  /** Cuánto lleva desplazada la lista, en puntos. Lo cuenta `(scroll)`. */
  readonly alto = signal(0)

  /** La fila tocada, para que se vea que el dedo también sigue valiendo. */
  readonly marcada = signal(0)

  readonly filas = [
    { numero: 1, texto: 'una' },
    { numero: 2, texto: 'dos' },
    { numero: 3, texto: 'tres' },
    { numero: 4, texto: 'cuatro' },
    { numero: 5, texto: 'cinco' },
    { numero: 6, texto: 'seis' },
    { numero: 7, texto: 'siete' },
    { numero: 8, texto: 'ocho' },
    { numero: 9, texto: 'nueve' },
    { numero: 10, texto: 'diez' },
    { numero: 11, texto: 'once' },
    { numero: 12, texto: 'doce' }
  ]
}
