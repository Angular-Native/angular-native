import { ChangeDetectionStrategy, Component, computed, signal } from '@angular/core'
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
 * 3. **El resalte del foco lo pinta cada control, no el sistema.** `an-button`
 *    es un `UIButton` y se levanta solo al enfocarse, porque lo dibuja UIKit.
 *    Un `an-view` con `(press)` es enfocable —el host lo hace posible— pero no
 *    se resalta: tvOS no tiene `UIFocusEffect`. Hoy no hay forma de saberlo
 *    desde la plantilla, y eso está apuntado como lo que falta.
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
      [style.gap]="'32'"
      [backgroundColor]="'#0b1020'">

      <an-text [fontSize]="76" [fontWeight]="'bold'" [color]="'#f4f7ff'">angular-native</an-text>

      <an-text [fontSize]="32" [color]="'#9fb0d4'">
        Angular con señales, en QuickJS, con el layout de taffy, sobre UIView de
        verdad. El mismo host que el teléfono; lo que cambia es que aquí se
        navega con el mando.
      </an-text>

      <!--
        Un botón del sistema. Es un UIButton, así que tvOS ya sabe enfocarlo y
        lo levanta él al llegarle el foco. Es lo primero que el mando encuentra.
      -->
      <an-button
        [title]="etiqueta()"
        [fontSize]="34"
        [color]="'#0b1020'"
        [backgroundColor]="'#6ee7b7'"
        [borderRadius]="16"
        [style.width]="'520'"
        [style.height]="'88'"
        (press)="pulsa()"></an-button>

      <!--
        Una vista pelada con (press). En iOS esto es un UIView con un
        UITapGestureRecognizer y ya está. En tvOS haría falta que además
        respondiera que sí a canBecomeFocused, y una UIView responde que no:
        el host la crea como AnFocusableView para que el mando pueda pararse
        aquí. Sin eso, este rectángulo sería inalcanzable.
      -->
      <an-view
        [style.width]="'520'"
        [style.height]="'88'"
        [borderRadius]="16"
        [backgroundColor]="'#1e2a4a'"
        (press)="pulsa()">
        <an-text
          [style.width]="'520'"
          [style.height]="'88'"
          [fontSize]="30"
          [textAlign]="'center'"
          [color]="'#f4f7ff'">y esto es un an-view, no un botón</an-text>
      </an-view>

      <an-text [fontSize]="26" [color]="'#5f7099'">
        llevas {{ segundos() }} segundos aquí
      </an-text>
    </an-view>
  `
})
export class AppComponent {
  readonly pulsaciones = signal(0)
  readonly segundos = signal(0)

  readonly etiqueta = computed(() =>
    this.pulsaciones() === 0 ? 'pulsa el botón central' : `pulsaciones: ${this.pulsaciones()}`
  )

  pulsa(): void {
    this.pulsaciones.update((valor) => valor + 1)
  }

  constructor() {
    // El reloj de JS lo marca el frame, no un hilo aparte: esto avanza con el
    // CADisplayLink del shell, igual que en el teléfono.
    setInterval(() => this.segundos.update((valor) => valor + 1), 1000)
  }
}
