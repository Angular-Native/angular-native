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
  row: T | undefined
  context: VirtualListContext<T> | null
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
 * Requiere altura de fila fija: sin ella no se puede saber qué hay en el
 * desplazamiento Y sin haber medido todo lo anterior.
 *
 * ```html
 * <VirtualList [items]="rows()" [itemHeight]="64" [style.flexGrow]="'1'">
 *   <ng-template let-row let-i="index">
 *     <Text>{{ i }}: {{ row.name }}</Text>
 *   </ng-template>
 * </VirtualList>
 * ```
 */
@Component({
  selector: 'VirtualList',
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
    <ScrollView
      [style.flexGrow]="'1'"
      [style.overflow]="'scroll'"
      [refreshing]="refreshing()"
      (refresh)="refresh.emit()"
      (layout)="onLayout($event)"
      (scroll)="onScroll($event)">
      <View [style.height]="totalHeight()" [style.position]="'relative'">
        @for (slot of slots(); track slot.key) {
          <View
            [style.position]="'absolute'"
            [style.top]="slot.top"
            [style.left]="'0'"
            [style.width]="'100%'"
            [style.height]="itemHeight()"
            [style.display]="slot.context ? 'flex' : 'none'">
            @if (slot.context) {
              <ng-container
                [ngTemplateOutlet]="template()!"
                [ngTemplateOutletContext]="slot.context" />
            }
          </View>
        }
      </View>
    </ScrollView>
  `
})
export class VirtualList<T> {
  readonly items = input.required<readonly T[]>()
  readonly itemHeight = input.required<number>()
  /** Ranuras de más a cada lado, para que un scroll rápido no deje huecos. */
  readonly overscan = input(4)

  /** Si está recargando. El gesto la abre; ponerla a `false` la cierra. */
  readonly refreshing = input(false)

  /** Tirar para recargar. Sin nadie escuchando, el gesto no existe. */
  readonly refresh = output<void>()

  protected readonly template = contentChild(TemplateRef<VirtualListContext<T>>)

  private readonly offset = signal(0)
  private readonly viewport = signal(0)

  protected readonly totalHeight = computed(() => this.items().length * this.itemHeight())

  /**
   * Cuántas ranuras hay. Solo cambia si cambia el alto del viewport o el de
   * las filas; desplazarse no la mueve, que es justo lo que permite reciclar.
   */
  private readonly slotCount = computed(() => {
    const visible = Math.ceil(this.viewport() / this.itemHeight())
    // Sin alto de viewport todavía no se sabe cuántas caben; se montan unas
    // pocas para que el primer frame no salga vacío.
    return (visible > 0 ? visible : 1) + this.overscan() * 2
  })

  protected readonly slots = computed<Slot<T>[]>(() => {
    const height = this.itemHeight()
    const items = this.items()
    const count = slotCountFor(this.slotCount(), items.length)
    const first = Math.max(
      0,
      Math.min(
        Math.floor(this.offset() / height) - this.overscan(),
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
        top: String(index * height),
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
