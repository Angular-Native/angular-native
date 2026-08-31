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

/** Lo que recibe la plantilla de cada fila. */
export interface VirtualListContext<T> {
  $implicit: T
  index: number
}

/**
 * Lista con ventana: solo monta las filas que se ven.
 *
 * Un `@for` sobre diez mil elementos crea diez mil `UIView`. Aquí se montan
 * las visibles más un margen, y cada fila se coloca en posición absoluta a
 * `index * itemHeight`. El contenedor lleva la altura total, así que el
 * `contentSize` del `UIScrollView` sale del layout sin cálculos aparte y la
 * barra de scroll mide lo que tiene que medir.
 *
 * **Esto es ventana, no reciclado de celdas.** Al salir de la ventana, la
 * vista se destruye; no se reutiliza como haría un `UITableView`. Reciclar
 * exigiría reasignar el contexto de una vista de Angular ya creada, y eso es
 * un problema distinto y bastante más grande.
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
  // El host tampoco puede dimensionarse por el contenido, o se lleva por
  // delante el layout del padre igual que haría el ScrollView de dentro.
  host: {
    '[style.minHeight]': "'0'",
    '[style.flexBasis]': "'0'",
    '[style.flexShrink]': "'1'",
    '[style.overflow]': "'hidden'"
  },
  imports: [ScrollView, View, NgTemplateOutlet],
  template: `
    <ScrollView
      [style.flexGrow]="'1'"
      [style.overflow]="'scroll'"
      (layout)="onLayout($event)"
      (scroll)="onScroll($event)">
      <View [style.height]="totalHeight()" [style.position]="'relative'">
        @for (row of window(); track row.index) {
          <View
            [style.position]="'absolute'"
            [style.top]="row.top"
            [style.left]="'0'"
            [style.width]="'100%'"
            [style.height]="itemHeight()">
            <ng-container
              [ngTemplateOutlet]="template()!"
              [ngTemplateOutletContext]="row.context" />
          </View>
        }
      </View>
    </ScrollView>
  `
})
export class VirtualList<T> {
  readonly items = input.required<readonly T[]>()
  readonly itemHeight = input.required<number>()
  /** Filas de más a cada lado, para que un scroll rápido no deje huecos. */
  readonly overscan = input(4)

  protected readonly template = contentChild(TemplateRef<VirtualListContext<T>>)

  private readonly offset = signal(0)
  private readonly viewport = signal(0)

  protected readonly totalHeight = computed(() => this.items().length * this.itemHeight())

  protected readonly window = computed(() => {
    const height = this.itemHeight()
    const items = this.items()
    // Sin altura de viewport todavía no se sabe cuántas caben; se monta un
    // puñado para que el primer frame no salga vacío.
    const visible = this.viewport() > 0 ? Math.ceil(this.viewport() / height) : this.overscan()
    const first = Math.max(0, Math.floor(this.offset() / height) - this.overscan())
    const last = Math.min(items.length, first + visible + this.overscan() * 2)

    const rows = []
    for (let index = first; index < last; index++) {
      rows.push({
        index,
        top: String(index * height),
        context: { $implicit: items[index], index } satisfies VirtualListContext<T>
      })
    }
    return rows
  })

  protected onScroll(event: NativeScrollEvent): void {
    this.offset.set(event.y)
  }

  protected onLayout(event: NativeLayoutEvent): void {
    this.viewport.set(event.height)
  }
}
