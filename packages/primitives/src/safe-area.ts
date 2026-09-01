import { ChangeDetectionStrategy, Component, computed, input, signal } from '@angular/core'

import { View, type NativeSafeAreaInsets } from './primitives'

/**
 * Aparta el contenido de lo que el sistema se reserva: el notch, la barra de
 * estado, el indicador de inicio, la barra de navegación de Android.
 *
 * Los márgenes no son constantes ni se pueden calcular: cambian al rotar, al
 * abrirse el teclado y al entrar en pantalla dividida. Los cuenta el host cada
 * vez que cambian.
 *
 * ```html
 * <SafeArea [edges]="['top', 'bottom']">
 *   <Text>ya no queda debajo del notch</Text>
 * </SafeArea>
 * ```
 */
@Component({
  selector: 'SafeArea',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [View],
  host: { '[style.flexGrow]': "'1'", '[style.minHeight]': "'0'" },
  template: `
    <View
      [style.flexGrow]="'1'"
      [style.paddingTop]="padding().top"
      [style.paddingRight]="padding().right"
      [style.paddingBottom]="padding().bottom"
      [style.paddingLeft]="padding().left"
      (safeArea)="insets.set($event)">
      <ng-content />
    </View>
  `
})
export class SafeArea {
  /** Qué bordes apartar. Por defecto, los cuatro. */
  readonly edges = input<readonly ('top' | 'right' | 'bottom' | 'left')[]>([
    'top',
    'right',
    'bottom',
    'left'
  ])

  protected readonly insets = signal<NativeSafeAreaInsets>({
    top: 0,
    right: 0,
    bottom: 0,
    left: 0
  })

  protected readonly padding = computed(() => {
    const insets = this.insets()
    const edges = this.edges()
    const only = (edge: 'top' | 'right' | 'bottom' | 'left') =>
      String(edges.includes(edge) ? insets[edge] : 0)
    return {
      top: only('top'),
      right: only('right'),
      bottom: only('bottom'),
      left: only('left')
    }
  })
}
