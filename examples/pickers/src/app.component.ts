import { ChangeDetectionStrategy, Component, signal } from '@angular/core'
import { NATIVE_PRIMITIVES, SafeArea } from '@angular-native/primitives'
import type {
  NativeIndexEvent,
  NativeTextEvent,
  NativeValueEvent
} from '@angular-native/primitives'

/**
 * Los controles de elegir: segmentos, desplegable, pasos, búsqueda y fecha.
 *
 * Todos son del sistema donde el sistema los tiene. Los dos que Android no
 * trae en la plataforma —el segmentado y el de pasos— se dibujan con vistas
 * del sistema respetando su aspecto actual, y se dice cuál es cuál en vez de
 * hacer como si fueran nativos.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, SafeArea],
  template: `
    <an-view [style.width]="'100%'" [style.height]="'100%'" [backgroundColor]="'#0b1020'">
      <an-safe-area [style.flex]="1" [padding]="20" [style.gap]="18">
        <an-text [fontSize]="26" [fontWeight]="700" [color]="'#f8fafc'">elegir</an-text>

        <an-search-bar
          [style.height]="52"
          [placeholder]="'Buscar…'"
          (input)="onSearch($event)" />
        <an-text [color]="'#94a3b8'" [fontSize]="14">{{ busqueda() || 'sin buscar nada' }}</an-text>

        <an-segmented-control
          [style.height]="36"
          [items]="vistas"
          [selectedIndex]="vista()"
          [color]="'#6ee7b7'"
          (change)="onVista($event)" />
        <an-text [color]="'#94a3b8'" [fontSize]="14">vista: {{ vistas[vista()] }}</an-text>

        <an-view [style.flexDirection]="'row'" [style.alignItems]="'center'" [style.gap]="'16'">
          <an-text [color]="'#cbd5f5'" [style.flexGrow]="'1'">Prioridad</an-text>
          <an-select
            [style.width]="150"
            [style.height]="40"
            [items]="prioridades"
            [selectedIndex]="prioridad()"
            (change)="prioridad.set($event.index)" />
        </an-view>

        <an-view [style.flexDirection]="'row'" [style.alignItems]="'center'" [style.gap]="'16'">
          <an-text [color]="'#cbd5f5'" [style.flexGrow]="'1'">Cantidad: {{ cantidad() }}</an-text>
          <an-stepper
            [style.width]="140"
            [style.height]="40"
            [value]="cantidad()"
            [minimumValue]="0"
            [maximumValue]="10"
            (change)="cantidad.set($event.value)" />
        </an-view>

        <an-view [style.flexDirection]="'row'" [style.alignItems]="'center'" [style.gap]="'16'">
          <an-text [color]="'#cbd5f5'" [style.flexGrow]="'1'">Fecha</an-text>
          <an-date-picker
            [style.width]="180"
            [style.height]="40"
            [value]="fecha()"
            (change)="fecha.set($event.value)" />
        </an-view>

        <!-- Las tres variantes de botón, que es lo que las separa de un texto. -->
        <an-view [style.flexDirection]="'row'" [style.gap]="'12'" [style.height]="48">
          <an-button
            [style.flexGrow]="'1'"
            [title]="'Texto'"
            [color]="'#6ee7b7'"
            (press)="pulsado.set('texto')"></an-button>
          <an-button
            [style.flexGrow]="'1'"
            [title]="'Tonal'"
            [variant]="'tonal'"
            [color]="'#6ee7b7'"
            (press)="pulsado.set('tonal')"></an-button>
          <an-button
            [style.flexGrow]="'1'"
            [title]="'Relleno'"
            [variant]="'filled'"
            [color]="'#6ee7b7'"
            (press)="pulsado.set('relleno')"></an-button>
        </an-view>
        <an-text [color]="'#94a3b8'" [fontSize]="14">último botón: {{ pulsado() }}</an-text>
      </an-safe-area>
    </an-view>
  `
})
export class AppComponent {
  readonly vistas = ['Día', 'Semana', 'Mes']
  readonly prioridades = ['Baja', 'Normal', 'Alta']

  readonly busqueda = signal('')
  readonly vista = signal(1)
  readonly prioridad = signal(1)
  readonly cantidad = signal(3)
  readonly fecha = signal(Date.now())
  readonly pulsado = signal('ninguno')

  onSearch(event: NativeTextEvent): void {
    this.busqueda.set(event.value)
  }

  onVista(event: NativeIndexEvent): void {
    this.vista.set(event.index)
  }

  onCantidad(event: NativeValueEvent): void {
    this.cantidad.set(event.value)
  }
}
