import { ChangeDetectionStrategy, Component, computed, signal } from '@angular/core'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'

/**
 * Lo que el reloj sabe pintar, en tres pantallas.
 *
 * Van en un `<an-stack-view>` y no una debajo de otra porque en 208 puntos de
 * ancho no cabe todo, y porque así se ve además la pila: solo se pinta la
 * pantalla de arriba, y entra y sale deslizándose.
 *
 * Todos los controles son los del sistema. Un `<an-switch>` es un `Toggle`, un
 * `<an-select>` es la rueda que gira con la corona, y un `<an-date-picker>` abre
 * el selector de esferas del propio watchOS: no hay ni un dibujo que se les
 * parezca.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-stack-view
      [style.width]="'100%'"
      [style.flexGrow]="'1'"
      [backgroundColor]="'#0b1020'"
      [transition]="sentido()">

      @if (pagina() === 0) {
        <an-view [style.width]="'100%'" [style.height]="'100%'">
          <an-scroll-view [style.width]="'100%'" [style.flexGrow]="'1'">
            <an-view
              [style.paddingTop]="'40'"
              [style.paddingHorizontal]="'10'"
              [style.paddingBottom]="'16'"
              [style.gap]="'10'"
              [style.width]="'100%'">

              <an-text [fontSize]="17" [fontWeight]="'bold'" [color]="'#f4f7ff'">controles</an-text>

              <!--
                El rótulo y el control son dos nodos, no uno: quien reparte el
                ancho es taffy, y un control con rótulo dentro sería SwiftUI
                decidiendo el layout por su cuenta.
              -->
              <an-view [style.flexDirection]="'row'" [style.alignItems]="'center'" [style.width]="'100%'">
                <an-text [style.flexGrow]="'1'" [fontSize]="14" [color]="'#9fb0d4'">aviso</an-text>
                <an-switch [style.width]="'60'" [(on)]="aviso" [color]="'#6ee7b7'"></an-switch>
              </an-view>

              <an-text [fontSize]="14" [color]="'#9fb0d4'">brillo {{ brillo().toFixed(0) }}%</an-text>
              <an-slider
                [style.width]="'100%'"
                [(value)]="brillo"
                [minimumValue]="0"
                [maximumValue]="100"
                [color]="'#6ee7b7'"></an-slider>
              <an-progress-bar [style.width]="'100%'" [progress]="brillo() / 100" [color]="'#6ee7b7'"></an-progress-bar>

              <an-text [fontSize]="14" [color]="'#9fb0d4'">tandas {{ tandas() }}</an-text>
              <an-stepper
                [style.width]="'100%'"
                [value]="tandas()"
                [minimumValue]="0"
                [maximumValue]="12"
                [step]="1"
                (change)="tandas.set($event.value)"></an-stepper>

              <an-view [style.flexDirection]="'row'" [style.alignItems]="'center'" [style.gap]="'10'" [style.width]="'100%'">
                <an-activity-indicator [color]="'#f59e0b'"></an-activity-indicator>
                <an-icon [name]="'favorite'" [size]="22" [color]="'#f472b6'"></an-icon>
                <an-icon [name]="'bell'" [size]="22" [color]="'#60a5fa'"></an-icon>
                <an-text [style.flexGrow]="'1'" [fontSize]="12" [color]="'#64748b'">SF Symbols</an-text>
              </an-view>

              <an-button
                [title]="'entrada →'"
                [color]="'#0b1020'"
                [backgroundColor]="'#6ee7b7'"
                [borderRadius]="10"
                (press)="ir(1)"></an-button>
            </an-view>
          </an-scroll-view>
        </an-view>
      }

      @if (pagina() === 1) {
        <an-view [style.width]="'100%'" [style.height]="'100%'">
          <an-scroll-view [style.width]="'100%'" [style.flexGrow]="'1'">
            <an-view
              [style.paddingTop]="'40'"
              [style.paddingHorizontal]="'10'"
              [style.paddingBottom]="'16'"
              [style.gap]="'10'"
              [style.width]="'100%'">

              <an-text [fontSize]="17" [fontWeight]="'bold'" [color]="'#f4f7ff'">entrada</an-text>

              <!--
                En el reloj un campo de texto no se escribe en su sitio: al
                tocarlo el sistema abre su pantalla —dictado, garabateo o
                teclado— y devuelve el resultado.
              -->
              <!--
                El alto va a mano y no lo pone el texto: en el reloj el campo
                trae su propio contenedor, más alto que una línea, y sin esto
                se solaparía con lo de debajo. El host lo dice por el registro
                si se olvida.
              -->
              <an-text-input
                [style.width]="'100%'"
                [style.height]="'44'"
                [placeholder]="'nombre'"
                [(value)]="nombre"
                [color]="'#f4f7ff'"
                [fontSize]="15"></an-text-input>
              <an-text [fontSize]="12" [color]="'#64748b'">hola, {{ nombre() || 'nadie' }}</an-text>

              <an-text [fontSize]="14" [color]="'#9fb0d4'">ritmo: {{ ritmos[ritmo()] }}</an-text>
              <an-select
                [style.width]="'100%'"
                [items]="ritmos"
                [selectedIndex]="ritmo()"
                (change)="ritmo.set($event.index)"></an-select>

              <an-text [fontSize]="14" [color]="'#9fb0d4'">a las</an-text>
              <an-date-picker
                [style.width]="'100%'"
                [mode]="'time'"
                [value]="hora()"
                (change)="hora.set($event.value)"></an-date-picker>

              <an-button
                [title]="'corona →'"
                [color]="'#0b1020'"
                [backgroundColor]="'#60a5fa'"
                [borderRadius]="10"
                (press)="ir(2)"></an-button>
            </an-view>
          </an-scroll-view>
        </an-view>
      }

      @if (pagina() === 2) {
        <!--
          Esta pantalla no lleva an-scroll-view a propósito. En el reloj la
          corona la tiene quien tiene el foco, y un ScrollView se la queda hasta
          que se toca otra cosa: sin él, la caja de abajo la coge sola al
          aparecer.
        -->
        <an-view [style.width]="'100%'" [style.height]="'100%'">
            <an-view
              [style.paddingTop]="'40'"
              [style.paddingHorizontal]="'10'"
              [style.gap]="'8'"
              [style.width]="'100%'">

              <an-text [fontSize]="17" [fontWeight]="'bold'" [color]="'#f4f7ff'">corona</an-text>

              <!--
                (crown) no lo declara ninguna directiva: es un oyente que
                Angular pasa tal cual al renderer, y el host del reloj lo
                convierte en digitalCrownRotation. Funciona, pero por eso hace
                falta el $any: sin una salida declarada en NativeVisual,
                Angular tipa el $event como Event a secas. Cuando la salida
                exista, el $any se cae solo.

                El (swipeLeft) y el (longPress) van sobre la misma caja para
                que se vea que conviven con la corona.
              -->
              <an-view
                [style.width]="'100%'"
                [style.height]="'64'"
                [style.alignItems]="'center'"
                [style.justifyContent]="'center'"
                [borderRadius]="12"
                [backgroundColor]="'#152036'"
                (crown)="gira($any($event))"
                (longPress)="aCero()"
                (swipeLeft)="ir(1)">
                <an-text [fontSize]="26" [fontWeight]="'bold'" [color]="'#f59e0b'">{{ pasos() }}</an-text>
                <an-text [fontSize]="11" [color]="'#64748b'">gira la corona</an-text>
              </an-view>
              <an-text [fontSize]="11" [color]="'#64748b'">
                velocidad {{ velocidad().toFixed(2) }} · mantén pulsado para poner a cero
              </an-text>

              <an-button
                [title]="'avisar'"
                [color]="'#0b1020'"
                [backgroundColor]="'#f59e0b'"
                [borderRadius]="10"
                (press)="dialogo.set(true)"></an-button>

              <an-button
                [title]="'detalle'"
                [color]="'#f4f7ff'"
                [backgroundColor]="'#2b1e4a'"
                [borderRadius]="10"
                (press)="hoja.set(true)"></an-button>

              <an-button
                [title]="'← controles'"
                [color]="'#9fb0d4'"
                [backgroundColor]="'#152036'"
                [borderRadius]="10"
                (press)="ir(0)"></an-button>
            </an-view>
        </an-view>
      }
    </an-stack-view>

    <!--
      El diálogo y la hoja no ocupan sitio: los presenta el sistema encima de
      todo. Por eso pueden estar aquí, fuera de la pila, y da igual qué página
      se esté viendo.
    -->
    <an-alert
      [visible]="dialogo()"
      [title]="'batería'"
      [message]="'quedan ' + brillo().toFixed(0) + ' por ciento'"
      [buttons]="respuestas"
      (select)="responde($event)"></an-alert>

    <!--
      Absoluto y a pantalla completa. Un an-modal es un nodo normal de cara al
      layout, así que si se deja en el flujo se come su trozo de la columna
      —en un reloj, la mitad de la pantalla— aunque no esté visible. El marco
      que se le dé aquí es además el tamaño con el que taffy coloca lo de
      dentro, y una hoja del reloj ocupa la pantalla entera.
    -->
    <an-modal
      [style.position]="'absolute'"
      [style.top]="'0'"
      [style.left]="'0'"
      [style.width]="'100%'"
      [style.height]="'100%'"
      [visible]="hoja()"
      [presentation]="'sheet'"
      (dismiss)="hoja.set(false)">
      <an-view
        [style.width]="'100%'"
        [style.height]="'100%'"
        [style.paddingTop]="'40'"
        [style.paddingHorizontal]="'12'"
        [style.gap]="'8'"
        [backgroundColor]="'#101827'">
        <an-text [fontSize]="16" [fontWeight]="'bold'" [color]="'#f4f7ff'">detalle</an-text>
        <an-text [fontSize]="13" [color]="'#9fb0d4'">
          Esto es una hoja del sistema, no una capa dibujada encima: se baja con el dedo.
        </an-text>
        <an-text [fontSize]="12" [color]="'#64748b'">última respuesta: {{ respuesta() }}</an-text>
        <an-button
          [title]="'cerrar'"
          [color]="'#0b1020'"
          [backgroundColor]="'#6ee7b7'"
          [borderRadius]="10"
          (press)="hoja.set(false)"></an-button>
      </an-view>
    </an-modal>
  `
})
export class AppComponent {
  readonly pagina = signal(0)
  readonly sentido = signal<'push' | 'pop'>('push')

  readonly aviso = signal(true)
  readonly brillo = signal(40)
  readonly tandas = signal(3)

  readonly nombre = signal('')
  readonly ritmos = ['suave', 'normal', 'fuerte']
  readonly ritmo = signal(1)
  readonly hora = signal(Date.now())

  /**
   * Pasos que lleva la corona.
   *
   * El acumulado se guarda con decimales y solo se redondea al enseñarlo: si se
   * redondeara al sumar, media muesca se perdería en cada aviso y girar despacio
   * no movería el número nunca.
   */
  private giro = 0
  readonly pasos = signal(0)
  readonly velocidad = signal(0)

  readonly dialogo = signal(false)
  readonly hoja = signal(false)
  readonly respuestas = ['vale', 'ahora no']
  /** Posición del botón pulsado en el diálogo; -1 mientras no se ha pulsado. */
  readonly elegido = signal(-1)
  readonly respuesta = computed(() =>
    this.elegido() < 0 ? 'ninguna' : this.respuestas[this.elegido()]
  )

  /**
   * Un aviso de la corona.
   *
   * Llega `delta` —cuánto ha girado desde el aviso anterior— y no solo el
   * acumulado, que es lo que casi siempre se quiere: sumar el paso a lo que ya
   * había sin tener que acordarse de dónde estaba.
   */
  gira(evento: { delta: number; offset: number; velocity: number }): void {
    this.giro = Math.max(0, this.giro + evento.delta)
    this.pasos.set(Math.round(this.giro))
    this.velocidad.set(evento.velocity)
  }

  aCero(): void {
    this.giro = 0
    this.pasos.set(0)
  }

  /** El botón de un `<an-alert>` llega como su posición en la lista. */
  responde(posicion: number): void {
    this.elegido.set(posicion)
    this.dialogo.set(false)
  }

  ir(destino: number): void {
    this.sentido.set(destino > this.pagina() ? 'push' : 'pop')
    this.pagina.set(destino)
  }
}
