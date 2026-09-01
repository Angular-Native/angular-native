import { ChangeDetectionStrategy, Component, computed, input, signal } from '@angular/core'

import { type NativeSafeAreaInsets } from './primitives'

/**
 * Aparta el contenido de lo que el sistema se reserva: el notch, la barra de
 * estado, el indicador de inicio, la barra de navegación de Android.
 *
 * Los márgenes no son constantes ni se pueden calcular: cambian al rotar, al
 * abrirse el teclado y al entrar en pantalla dividida. Los cuenta el host cada
 * vez que cambian.
 *
 * ```html
 * <SafeArea [edges]="['top', 'bottom']" [padding]="20" [style.gap]="'12'">
 *   <Text>ya no queda debajo del notch</Text>
 * </SafeArea>
 * ```
 *
 * No hay una vista dentro: el área segura *es* su vista, así que lo que se le
 * ponga para ordenar a los hijos —`gap`, `flexDirection`, `alignItems`— manda
 * sobre ellos. Con una vista intermedia no lo hacía: los estilos se quedaban
 * en el envoltorio, que solo tenía un hijo, y no pasaba nada. Sin ruido, sin
 * error, sin separación.
 *
 * El apartado se aplica como relleno y no como margen a propósito: los
 * márgenes que reserva el sistema se cuentan para la vista donde está, así que
 * una vista que se apartara con margen dejaría de estar debajo del notch,
 * pasaría a reservar cero, volvería a su sitio, y así sin parar.
 */
@Component({
  selector: 'SafeArea',
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    '[style.flexGrow]': "'1'",
    '[style.minHeight]': "'0'",
    '[style.paddingTop]': 'space().top',
    '[style.paddingRight]': 'space().right',
    '[style.paddingBottom]': 'space().bottom',
    '[style.paddingLeft]': 'space().left',
    '(safeArea)': 'onInsets($event)'
  },
  template: `<ng-content />`
})
export class SafeArea {
  /** Qué bordes apartar. Por defecto, los cuatro. */
  readonly edges = input<readonly ('top' | 'right' | 'bottom' | 'left')[]>([
    'top',
    'right',
    'bottom',
    'left'
  ])

  /**
   * Relleno propio, que se suma al que reserva el sistema.
   *
   * Va como entrada y no como `[style.padding]` porque el relleno de esta
   * vista ya lo escribe el área segura: los dos a la vez se pisarían, y el
   * que perdiera lo haría en silencio.
   */
  readonly padding = input(0)

  /**
   * En una escucha del host, `$event` está tipado como `Event` y no hay forma
   * de decirle a Angular que este trae otra cosa. El casteo vive aquí, en un
   * sitio, en vez de en cada plantilla.
   */
  protected onInsets(event: Event): void {
    this.insets.set(event as unknown as NativeSafeAreaInsets)
  }

  protected readonly insets = signal<NativeSafeAreaInsets>({
    top: 0,
    right: 0,
    bottom: 0,
    left: 0
  })

  protected readonly space = computed(() => {
    const insets = this.insets()
    const edges = this.edges()
    const extra = this.padding()
    const only = (edge: 'top' | 'right' | 'bottom' | 'left') =>
      String((edges.includes(edge) ? insets[edge] : 0) + extra)
    return {
      top: only('top'),
      right: only('right'),
      bottom: only('bottom'),
      left: only('left')
    }
  })
}
