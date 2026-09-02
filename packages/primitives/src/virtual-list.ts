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
 * The rows' height: one for all of them, or one per row.
 *
 * The function is called once per row every time the list changes, not on every
 * scroll.
 */
export type ItemHeight<T> = number | ((item: T, index: number) => number)

/** What each row's template receives. */
export interface VirtualListContext<T> {
  $implicit: T
  index: number
}

/** One slot in the carousel. Its `key` never changes; its content does. */
interface Slot<T> {
  key: number
  index: number
  top: string
  height: string
  row: T | undefined
  context: VirtualListContext<T> | null
}

/**
 * Where each row starts and how tall it is.
 *
 * With a single height nothing needs storing: row `i`'s position is
 * `i * height` and the row at a given offset comes out of a division. With
 * differing heights they have to be added up, so they are accumulated once per
 * list and then searched by bisection, which over five thousand rows is thirteen
 * comparisons.
 */
type Metrics =
  | { readonly kind: 'fixed'; readonly height: number; readonly min: number; readonly total: number }
  | { readonly kind: 'variable'; readonly starts: number[]; readonly min: number; readonly total: number }

function metricsFor<T>(items: readonly T[], height: ItemHeight<T>): Metrics {
  if (typeof height === 'number') {
    return { kind: 'fixed', height, min: height, total: items.length * height }
  }
  // `starts` has one entry more than there are rows: the last one is the total
  // height, so that row `i`'s slot is always `starts[i + 1] - starts[i]`.
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

/** The row that sits at that offset. */
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
 * A list that recycles its views.
 *
 * It mounts a fixed number of slots —as many as fit on screen plus a margin— and
 * scrolling creates and destroys none of them: it changes what each one shows
 * and where it sits. Ten thousand rows cost the same twenty native views as
 * twenty rows do.
 *
 * The trick is in the `track slot.key`: a slot's key is its position in the
 * carousel, not the item it is showing, so Angular reuses the embedded view and
 * only updates its bindings. `NgTemplateOutlet` does the same as long as the
 * context's keys do not change, which is the case here.
 *
 * The row height has to be given: without it there is no telling what sits at a
 * given offset without having measured everything before it. It can be one for
 * all of them or a function per row.
 *
 * ```html
 * <an-virtual-list [items]="rows()" [itemHeight]="64" [style.flexGrow]="'1'">
 *   <ng-template let-row let-i="index">
 *     <an-text>{{ i }}: {{ row.name }}</an-text>
 *   </ng-template>
 * </an-virtual-list>
 * ```
 *
 * With rows of differing heights, `itemHeight` takes a function instead:
 *
 * ```html
 * <an-virtual-list [items]="rows()" [itemHeight]="rowHeight" [style.flexGrow]="'1'">
 * ```
 */
@Component({
  selector: 'an-virtual-list',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [ScrollView, View, NgTemplateOutlet],
  // The host cannot be sized by its content either, or it takes the parent's
  // layout down with it just as the ScrollView inside would.
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
  /** Spare slots at each end, so a fast scroll leaves no gaps. */
  readonly overscan = input(4)

  /** Whether it is refreshing. The gesture opens it; setting it to `false`
   * closes it. */
  readonly refreshing = input(false)

  /** Pull to refresh. With nobody listening, the gesture does not exist. */
  readonly refresh = output<void>()

  protected readonly template = contentChild(TemplateRef<VirtualListContext<T>>)

  private readonly offset = signal(0)
  private readonly viewport = signal(0)

  private readonly metrics = computed(() => metricsFor(this.items(), this.itemHeight()))

  protected readonly totalHeight = computed(() => this.metrics().total)

  /**
   * How many slots there are. It only changes when the viewport's height or the
   * rows' does; scrolling does not move it, which is precisely what makes
   * recycling possible.
   */
  private readonly slotCount = computed(() => {
    // With differing heights the shortest one governs: it is what decides how
    // many rows fit in the worst case, and coming up short would leave gaps on
    // scroll.
    const visible = Math.ceil(this.viewport() / this.metrics().min)
    // With no viewport height yet there is no telling how many fit; a few are
    // mounted so the first frame does not come out empty.
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
        // The context's keys never change, which is why `NgTemplateOutlet`
        // updates the view instead of rebuilding it.
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
 * A list shorter than the carousel needs no empty slots: it gets trimmed. Going
 * from a short list to a long one does create slots, but that happens when
 * filtering, not when scrolling.
 */
function slotCountFor(desired: number, total: number): number {
  return Math.min(desired, Math.max(total, 1))
}
