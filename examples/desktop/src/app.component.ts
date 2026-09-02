import { ChangeDetectionStrategy, Component, signal } from '@angular/core'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'
import type { NativeCursor, NativeHoverEvent } from '@angular-native/primitives'

/**
 * Lo que solo existe en un escritorio: puntero, forma del puntero, y el gesto
 * de deslizar del trackpad.
 *
 * No hay ninguno de los tres en un teléfono, así que este ejemplo no está para
 * verse en el simulador: está para verse en un Mac, con un ratón encima. Las
 * tres cosas las entrega el sistema —`NSTrackingArea`, `NSCursor` y
 * `swipeWithEvent:`— y ninguna se dibuja aquí.
 *
 * La cabecera de arriba no se ve en el contenido a propósito: en macOS el
 * `[title]` de un `<an-navigation-bar>` acaba en la barra de título de la
 * ventana, que es donde un usuario de Mac lo busca, y el nodo no ocupa sitio.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-view
      [style.width]="'100%'"
      [style.height]="'100%'"
      [style.padding]="'20'"
      [style.gap]="'18'"
      [backgroundColor]="'#0b1020'">
      <an-navigation-bar [title]="titulo()" />

      <an-text [fontSize]="26" [fontWeight]="700" [color]="'#f8fafc'">escritorio</an-text>
      <an-text [fontSize]="13" [color]="'#94a3b8'">{{ pista() }}</an-text>

      <!-- Pasar por encima. El fondo, el borde y el rótulo cambian con
           (hover), y el puntero cambia con [cursor]. -->
      <an-text [fontSize]="15" [fontWeight]="600" [color]="'#cbd5e1'">encima</an-text>
      <an-view [style.flexDirection]="'row'" [style.gap]="'12'" [style.height]="86">
        @for (tarjeta of tarjetas; track tarjeta.nombre) {
          <an-view
            [style.flexGrow]="'1'"
            [style.alignItems]="'center'"
            [style.justifyContent]="'center'"
            [style.gap]="'6'"
            [borderRadius]="12"
            [borderWidth]="2"
            [borderColor]="encima() === tarjeta.nombre ? '#6ee7b7' : '#1e293b'"
            [backgroundColor]="encima() === tarjeta.nombre ? '#12324a' : '#111a2e'"
            [cursor]="tarjeta.cursor"
            (hover)="onHover(tarjeta.nombre, $event)">
            <an-text [fontSize]="15" [fontWeight]="600" [color]="'#e2e8f0'">
              {{ tarjeta.nombre }}
            </an-text>
            <an-text [fontSize]="12" [color]="'#94a3b8'">
              {{ encima() === tarjeta.nombre ? 'dentro' : 'fuera' }}
            </an-text>
          </an-view>
        }
      </an-view>

      <!-- Un control del sistema también sabe decir cuándo tiene el puntero
           encima: el área vigilada no la lleva la vista, la lleva un objeto
           aparte, así que no hace falta subclasear el NSButton. -->
      <an-button
        [style.height]="40"
        [title]="botonEncima() ? 'y un botón del sistema también' : 'pasa por aquí'"
        [variant]="'tonal'"
        [color]="'#6ee7b7'"
        [cursor]="'pointer'"
        (hover)="botonEncima.set($event.hovered)"
        (press)="pulsaciones.set(pulsaciones() + 1)"></an-button>

      <!-- Deslizar. Dos dedos en el trackpad, con el umbral del sistema. -->
      <an-text [fontSize]="15" [fontWeight]="600" [color]="'#cbd5e1'">deslizar</an-text>
      <an-view
        [style.flex]="1"
        [style.alignItems]="'center'"
        [style.justifyContent]="'center'"
        [style.gap]="'8'"
        [borderRadius]="14"
        [backgroundColor]="'#111a2e'"
        [cursor]="'grab'"
        (swipeLeft)="onSwipe('izquierda', '←')"
        (swipeRight)="onSwipe('derecha', '→')"
        (swipeUp)="onSwipe('arriba', '↑')"
        (swipeDown)="onSwipe('abajo', '↓')">
        <an-text [fontSize]="34" [fontWeight]="700" [color]="'#6ee7b7'">{{ flecha() }}</an-text>
        <an-text [fontSize]="14" [color]="'#e2e8f0'">{{ ultimoDeslizamiento() }}</an-text>
        <an-text [fontSize]="12" [color]="'#94a3b8'">
          deslizamientos: {{ deslizamientos() }} · pulsaciones: {{ pulsaciones() }}
        </an-text>
      </an-view>
    </an-view>
  `
})
export class AppComponent {
  /**
   * Cada tarjeta enseña un puntero del sistema distinto. Son los de
   * `NSCursor`, pedidos por el nombre de CSS.
   */
  readonly tarjetas: ReadonlyArray<{ nombre: string; cursor: NativeCursor }> = [
    { nombre: 'pointer', cursor: 'pointer' },
    { nombre: 'text', cursor: 'text' },
    { nombre: 'crosshair', cursor: 'crosshair' },
    { nombre: 'not-allowed', cursor: 'not-allowed' }
  ]

  readonly encima = signal<string | null>(null)
  readonly botonEncima = signal(false)
  readonly pulsaciones = signal(0)
  readonly deslizamientos = signal(0)
  readonly ultimoDeslizamiento = signal('todavía nada')

  readonly titulo = signal('escritorio · angular-native')

  readonly pista = signal(
    'pasa el ratón por las tarjetas y desliza con dos dedos en el recuadro de abajo'
  )

  readonly flecha = signal('·')

  onHover(nombre: string, evento: NativeHoverEvent): void {
    this.encima.set(evento.hovered ? nombre : null)
    if (evento.hovered) {
      // El punto llega en coordenadas de la vista, igual que el de un (press).
      console.log(`[hover] dentro de ${nombre} en ${Math.round(evento.x)},${Math.round(evento.y)}`)
    }
  }

  onSwipe(direccion: string, flecha: string): void {
    this.deslizamientos.set(this.deslizamientos() + 1)
    this.ultimoDeslizamiento.set(direccion)
    this.flecha.set(flecha)
    console.log(`[swipe] ${direccion}`)
  }
}
