import { NgTemplateOutlet } from '@angular/common'
import {
  ChangeDetectionStrategy,
  Component,
  computed,
  contentChild,
  input,
  signal,
  TemplateRef
} from '@angular/core'

import { ScrollView, View, type NativeLayoutEvent, type NativeScrollEvent } from './primitives'
import { output } from '@angular/core'

/**
 * El alto de las filas: uno para todas, o uno por fila.
 *
 * La función se llama una vez por fila cada vez que cambia la lista, no en
 * cada desplazamiento.
 */
export type ItemHeight<T> = number | ((item: T, index: number) => number)

/** Lo que recibe la plantilla de cada fila. */
export interface VirtualListContext<T> {
  $implicit: T
  index: number
}

/** Una ranura del carrusel. Su `key` no cambia nunca; su contenido sí. */
interface Slot<T> {
  key: number
  index: number
  top: string
  height: string
  row: T | undefined
  context: VirtualListContext<T> | null
}

/**
 * Dónde empieza cada fila y cuánto mide.
 *
 * Con un alto único no hace falta guardar nada: la posición de la fila `i` es
 * `i * alto` y la fila que hay en un desplazamiento sale de una división. Con
 * altos distintos hay que sumar, así que se acumulan una vez por lista y luego
 * se busca por bisección, que en cinco mil filas son trece comparaciones.
 */
type Metrics =
  | { readonly kind: 'fixed'; readonly height: number; readonly min: number; readonly total: number }
  | { readonly kind: 'variable'; readonly starts: number[]; readonly min: number; readonly total: number }

function metricsFor<T>(items: readonly T[], height: ItemHeight<T>): Metrics {
  if (typeof height === 'number') {
    return { kind: 'fixed', height, min: height, total: items.length * height }
  }
  // `starts` tiene una entrada más que filas: la última es el alto total, y
  // así el hueco de la fila `i` es siempre `starts[i + 1] - starts[i]`.
  const starts = new Array<number>(items.length + 1)
  starts[0] = 0
  let min = Infinity
  for (let i = 0; i < items.length; i++) {
    const one = height(items[i], i)
    starts[i + 1] = starts[i] + one
    if (one < min) {
      min = one
    }
  }
  return {
    kind: 'variable',
    starts,
    min: Number.isFinite(min) && min > 0 ? min : 1,
    total: starts[items.length]
  }
}

function topOf(metrics: Metrics, index: number): number {
  return metrics.kind === 'fixed' ? index * metrics.height : metrics.starts[index] ?? metrics.total
}

function heightOf(metrics: Metrics, index: number): number {
  if (metrics.kind === 'fixed') {
    return metrics.height
  }
  return (metrics.starts[index + 1] ?? metrics.total) - (metrics.starts[index] ?? metrics.total)
}

/** La fila que ocupa ese desplazamiento. */
function indexAt(metrics: Metrics, offset: number): number {
  if (metrics.kind === 'fixed') {
    return Math.floor(offset / metrics.height)
  }
  const starts = metrics.starts
  let low = 0
  let high = starts.length - 1
  while (low < high) {
    const mid = (low + high + 1) >> 1
    if (starts[mid] <= offset) {
      low = mid
    } else {
      high = mid - 1
    }
  }
  return low
}

/**
 * Lista con reciclado de vistas.
 *
 * Monta un número fijo de ranuras —las que caben en pantalla más un margen— y
 * al desplazarse no crea ni destruye ninguna: cambia lo que muestra cada una y
 * dónde está. Diez mil filas cuestan las mismas veinte vistas nativas que
 * veinte filas.
 *
 * El truco está en el `track slot.key`: la clave de una ranura es su posición
 * en el carrusel, no el elemento que enseña, así que Angular reutiliza la vista
 * incrustada y solo actualiza sus bindings. `NgTemplateOutlet` hace lo mismo
 * mientras las claves del contexto no cambien, que es el caso.
 *
 * El alto de fila hay que darlo: sin él no se sabe qué hay en un
 * desplazamiento sin haber medido todo lo anterior. Puede ser uno para todas
 * o una función por fila.
 *
 * ```html
 * <an-virtual-list [items]="rows()" [itemHeight]="64" [style.flexGrow]="'1'">
 * <an-virtual-list [items]="rows()" [itemHeight]="alto" [style.flexGrow]="'1'">
 *   <ng-template let-row let-i="index">
 *     <an-text>{{ i }}: {{ row.name }}</an-text>
 *   </ng-template>
 * </an-virtual-list>
 * ```
 */
@Component({
  selector: 'an-virtual-list',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [ScrollView, View, NgTemplateOutlet],
  // El host tampoco puede dimensionarse por el contenido, o se lleva por
  // delante el layout del padre igual que haría el ScrollView de dentro.
  host: {
    '[style.minHeight]': "'0'",
    '[style.flexBasis]': "'0'",
    '[style.flexShrink]': "'1'",
    '[style.overflow]': "'hidden'"
  },
  template: `
    <an-scroll-view
      [style.flexGrow]="'1'"
      [style.overflow]="'scroll'"
      [refreshing]="refreshing()"
      (refresh)="refresh.emit()"
      (layout)="onLayout($event)"
      (scroll)="onScroll($event)">
      <an-view [style.height]="totalHeight()" [style.position]="'relative'">
        @for (slot of slots(); track slot.key) {
          <an-view
            [style.position]="'absolute'"
            [style.top]="slot.top"
            [style.left]="'0'"
            [style.width]="'100%'"
            [style.height]="slot.height"
            [style.display]="slot.context ? 'flex' : 'none'">
            @if (slot.context) {
              <ng-container
                [ngTemplateOutlet]="template()!"
                [ngTemplateOutletContext]="slot.context" />
            }
          </an-view>
        }
      </an-view>
    </an-scroll-view>
  `
})
export class VirtualList<T> {
  readonly items = input.required<readonly T[]>()
  readonly itemHeight = input.required<ItemHeight<T>>()
  /** Ranuras de más a cada lado, para que un scroll rápido no deje huecos. */
  readonly overscan = input(4)

  /** Si está recargando. El gesto la abre; ponerla a `false` la cierra. */
  readonly refreshing = input(false)

  /** Tirar para recargar. Sin nadie escuchando, el gesto no existe. */
  readonly refresh = output<void>()

  protected readonly template = contentChild(TemplateRef<VirtualListContext<T>>)

  private readonly offset = signal(0)
  private readonly viewport = signal(0)

  private readonly metrics = computed(() => metricsFor(this.items(), this.itemHeight()))

  protected readonly totalHeight = computed(() => this.metrics().total)

  /**
   * Cuántas ranuras hay. Solo cambia si cambia el alto del viewport o el de
   * las filas; desplazarse no la mueve, que es justo lo que permite reciclar.
   */
  private readonly slotCount = computed(() => {
    // Con altos distintos manda el más bajo: es el que decide cuántas filas
    // caben en el peor caso, y quedarse corto dejaría huecos al desplazarse.
    const visible = Math.ceil(this.viewport() / this.metrics().min)
    // Sin alto de viewport todavía no se sabe cuántas caben; se montan unas
    // pocas para que el primer frame no salga vacío.
    return (visible > 0 ? visible : 1) + this.overscan() * 2
  })

  protected readonly slots = computed<Slot<T>[]>(() => {
    const metrics = this.metrics()
    const items = this.items()
    const count = slotCountFor(this.slotCount(), items.length)
    const first = Math.max(
      0,
      Math.min(
        indexAt(metrics, this.offset()) - this.overscan(),
        Math.max(0, items.length - count)
      )
    )

    const slots: Slot<T>[] = []
    for (let key = 0; key < count; key++) {
      const index = first + key
      const row = index < items.length ? items[index] : undefined
      slots.push({
        key,
        index,
        top: String(topOf(metrics, index)),
        height: String(heightOf(metrics, index)),
        row,
        // Las claves del contexto no cambian nunca, y por eso
        // `NgTemplateOutlet` actualiza la vista en vez de rehacerla.
        context: row === undefined ? null : { $implicit: row, index }
      })
    }
    return slots
  })

  protected onScroll(event: NativeScrollEvent): void {
    this.offset.set(event.y)
  }

  protected onLayout(event: NativeLayoutEvent): void {
    this.viewport.set(event.height)
  }
}

/**
 * Una lista más corta que el carrusel no necesita ranuras vacías: se recorta.
 * Pasar de una lista corta a una larga sí crea ranuras, pero eso ocurre al
 * filtrar, no al desplazarse.
 */
function slotCountFor(desired: number, total: number): number {
  return Math.min(desired, Math.max(total, 1))
}
