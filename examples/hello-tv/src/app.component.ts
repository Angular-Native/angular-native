import { ChangeDetectionStrategy, Component, signal } from '@angular/core'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'

/**
 * La misma clase de componente que corre en el teléfono, con las medidas y el
 * modelo de interacción de una tele.
 *
 * Tres cosas que no son cosméticas y que esta pantalla enseña a propósito:
 *
 * 1. **No hay toques.** Nada de lo que hay aquí se toca: el mando mueve el
 *    foco de una vista a otra y el botón central pulsa la que esté enfocada.
 *    Una vista que no puede recibir el foco no se puede pulsar, así que un
 *    `(press)` sobre algo que no sea `an-view` o un control del sistema no se
 *    dispara nunca. Ver `docs/tvos.md`.
 *
 * 2. **Los márgenes son de tele.** Los bordes de un televisor se recortan
 *    —overscan—, y Apple pide dejar 90 puntos a los lados y 60 arriba y abajo.
 *    El viewport son 1920x1080 puntos, no los 393 de un iPhone: un `fontSize`
 *    de 28 aquí no se lee desde el sofá.
 *
 * 3. **El resalte del foco lo pinta cada control, no el sistema.** Los dos
 *    `an-button` son `UIButton` y se levantan y se ponen blancos solos al
 *    enfocarse, porque eso lo dibuja UIKit: por eso hay dos, y no uno. Mover
 *    el foco entre ellos se ve. El `an-view` de abajo es enfocable —el host lo
 *    crea como `AnFocusableView`— y pulsable, pero no se resalta: tvOS no
 *    tiene `UIFocusEffect`, y aquí no se dibuja ninguno a mano. Que el foco
 *    llegó se ve en su contador al pulsar el botón central.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-view
      [style.width]="'100%'"
      [style.height]="'100%'"
      [style.paddingHorizontal]="'90'"
      [style.paddingVertical]="'60'"
      [style.gap]="'28'"
      [backgroundColor]="'#0b1020'">

      <an-text [fontSize]="76" [fontWeight]="'bold'" [color]="'#f4f7ff'">angular-native</an-text>

      <an-text [fontSize]="30" [color]="'#9fb0d4'">
        Angular con señales, en QuickJS, con el layout de taffy, sobre UIView de
        verdad. El mismo host que el teléfono; lo que cambia es que aquí se
        navega con el mando.
      </an-text>

      <!--
        Dos botones del sistema, uno debajo del otro. Son UIButton, así que
        tvOS ya sabe enfocarlos y es él quien los levanta y los pone blancos al
        llegarles el foco. Están para que mover el foco con el mando se vea en
        una captura sin dibujar nada.
      -->
      <an-button
        [title]="'arriba — pulsado ' + arriba() + ' veces'"
        [fontSize]="34"
        [color]="'#0b1020'"
        [backgroundColor]="'#6ee7b7'"
        [borderRadius]="16"
        [style.width]="'760'"
        [style.height]="'88'"
        (press)="arriba.set(arriba() + 1)"></an-button>

      <an-button
        [title]="'abajo — pulsado ' + abajo() + ' veces'"
        [fontSize]="34"
        [color]="'#0b1020'"
        [backgroundColor]="'#fca5a5'"
        [borderRadius]="16"
        [style.width]="'760'"
        [style.height]="'88'"
        (press)="abajo.set(abajo() + 1)"></an-button>

      <!--
        Una vista pelada con (press). En iOS esto es un UIView con un
        UITapGestureRecognizer y ya está. En tvOS haría falta que además
        respondiera que sí a canBecomeFocused, y una UIView responde que no:
        el host la crea como AnFocusableView para que el mando pueda pararse
        aquí. Sin eso, este rectángulo sería inalcanzable.
      -->
      <an-view
        [style.width]="'760'"
        [style.height]="'88'"
        [borderRadius]="16"
        [backgroundColor]="'#1e2a4a'"
        (press)="vista.set(vista() + 1)">
        <an-text
          [style.width]="'760'"
          [style.height]="'88'"
          [fontSize]="30"
          [textAlign]="'center'"
          [color]="'#f4f7ff'">an-view, no un botón — pulsada {{ vista() }} veces</an-text>
      </an-view>

      <an-text [fontSize]="26" [color]="'#5f7099'">
        llevas {{ segundos() }} segundos aquí
      </an-text>
    </an-view>
  `
})
export class AppComponent {
  readonly arriba = signal(0)
  readonly abajo = signal(0)
  /** El `an-view`. Su contador aparte es lo que demuestra que el foco llegó a
   *  una vista que no es un control: si se pulsara el botón, subiría el otro. */
  readonly vista = signal(0)
  readonly segundos = signal(0)

  constructor() {
    // El reloj de JS lo marca el frame, no un hilo aparte: esto avanza con el
    // CADisplayLink del shell, igual que en el teléfono.
    setInterval(() => this.segundos.update((valor) => valor + 1), 1000)
  }
}
