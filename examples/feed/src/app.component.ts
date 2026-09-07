import { ChangeDetectionStrategy, Component, computed, signal } from '@angular/core'
import {
  NATIVE_PRIMITIVES,
  SafeArea,
  type NativeLayoutEvent
} from '@angular-native/primitives'

interface Item {
  readonly id: number
  readonly title: string
  readonly author: string
  readonly saves: number
  readonly category: string
  /** How tall the picture is drawn. A waterfall is only a waterfall because the
   *  cards disagree about their height. */
  readonly height: number
}

const TITLES: readonly [string, string, string][] = [
  ['Cold harbour, 6 a.m.', 'ines.mar', 'Places'],
  ['Waxed cotton, twelve ounce', 'thread.co', 'Making'],
  ['A kitchen with one window', 'aoife', 'Places'],
  ['Sourdough, day four', 'pan.diario', 'Making'],
  ['Ferry across the ría', 'ines.mar', 'Places'],
  ['Repairing a chair', 'taller.gz', 'Making'],
  ['Fog on the estuary', 'aoife', 'Places'],
  ['Indigo, second dip', 'thread.co', 'Making'],
  ['The long platform', 'ines.mar', 'Places'],
  ['Two hundred grams of salt', 'pan.diario', 'Making'],
  ['Slate roofs after rain', 'aoife', 'Places'],
  ['Hand-cut dovetails', 'taller.gz', 'Making']
]

// Fixed seeds so the same twelve pictures come back on every run: a screenshot
// that changes each time it is taken is no use for comparing anything.
const ITEMS: readonly Item[] = TITLES.map(([title, author, category], index) => ({
  id: index,
  title,
  author,
  category,
  saves: 40 + ((index * 37) % 260),
  height: [200, 260, 150, 230, 180, 290][index % 6]
}))

const CATEGORIES = ['All', 'Places', 'Making'] as const

/**
 * A two-column gallery.
 *
 * There is no grid in the layout engine and there does not need to be: a
 * waterfall is two columns side by side, and the items are dealt out between
 * them. Flexbox resolves it in the Rust core, once, and both columns arrive at
 * the host as absolute frames.
 *
 * Everything around the pictures is the platform's: the search field is a
 * `UISearchBar` and an Android `SearchView`, the category strip is a
 * `UISegmentedControl` and a `MaterialButtonToggleGroup`, and pulling the list
 * down is the system's own refresh control.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, SafeArea],
  template: `
    <an-view [style.flexGrow]="'1'" [backgroundColor]="'#0b0e14'">
      <an-safe-area [edges]="['top', 'bottom']" [style.flexGrow]="'1'">
        <an-view [style.paddingHorizontal]="'16'" [style.paddingTop]="'6'" [style.gap]="'12'">
          <an-view
            [style.flexDirection]="'row'"
            [style.alignItems]="'center'"
            [style.justifyContent]="'space-between'">
            <an-text [fontSize]="28" [fontWeight]="'700'" [color]="'#f4f7ff'">Saved</an-text>
            <an-icon [name]="'more'" [size]="22" [color]="'#8a93a6'" />
          </an-view>

          <an-search-bar
            [value]="query()"
            [placeholder]="'Search saved'"
            [style.height]="'40'"
            (input)="query.set($event.value)" />

          <an-segmented-control
            [items]="categories"
            [selectedIndex]="category()"
            (change)="category.set($event.index)" />
        </an-view>

        <!-- The width is measured on the scroll view and handed down, and the
             row below is given it in points rather than being left to work it
             out. Measuring the row instead is a loop: a photograph that lands
             reports its intrinsic width into layout, the row grows, the wider
             row is reported back, and the columns grow with it until the cards
             are hanging off both sides of the screen. The scroll view's own
             width is the one thing on this screen that no content can move. -->
        <an-scroll-view
          [style.flexGrow]="'1'"
          [style.minHeight]="'0'"
          [style.overflow]="'scroll'"
          [refreshing]="refreshing()"
          (layout)="onViewportLayout($event)"
          (refresh)="onRefresh()">
          <an-view
            [style.width]="viewport()"
            [style.flexDirection]="'row'"
            [style.gap]="'12'"
            [style.padding]="'16'"
            [style.alignItems]="'flex-start'">
            @for (column of columns(); track $index) {
              <an-view [style.width]="columnWidth()" [style.gap]="'12'">
                @for (item of column; track item.id) {
                  <an-view
                    [backgroundColor]="'#141926'"
                    [borderRadius]="14"
                    [style.minWidth]="'0'"
                    (press)="open.set(item.title)">
                    <!-- The picture is positioned, not laid out.
                         When a photograph lands, its intrinsic size goes back
                         into layout, and a 600-point-wide image inside a
                         175-point column becomes a floor the column cannot get
                         under — the second column ends up off the screen.
                         An absolutely positioned child contributes nothing to
                         its parent's width, so the frame below is the one that
                         decides, and the photograph fills it. headless never
                         shows this, because headless never loads an image. -->
                    <an-view [style.height]="String(item.height)">
                      <an-image
                        [source]="photo(item.id)"
                        [resizeMode]="'cover'"
                        [style.position]="'absolute'"
                        [style.top]="'0'"
                        [style.left]="'0'"
                        [style.right]="'0'"
                        [style.bottom]="'0'" />
                    </an-view>
                    <an-view [style.padding]="'12'" [style.gap]="'8'">
                      <an-text [fontSize]="14" [lineHeight]="19" [color]="'#f4f7ff'"
                               [numberOfLines]="2">{{ item.title }}</an-text>
                      <an-view
                        [style.flexDirection]="'row'"
                        [style.alignItems]="'center'"
                        [style.justifyContent]="'space-between'">
                        <an-text [fontSize]="12" [color]="'#8a93a6'">{{ item.author }}</an-text>
                        <an-view
                          [style.flexDirection]="'row'"
                          [style.alignItems]="'center'"
                          [style.gap]="'4'">
                          <an-icon [name]="'favorite'" [size]="12" [color]="'#ff5a1f'" />
                          <an-text [fontSize]="12" [color]="'#8a93a6'">{{ item.saves }}</an-text>
                        </an-view>
                      </an-view>
                    </an-view>
                  </an-view>
                }
              </an-view>
            }
          </an-view>

          @if (visible().length === 0) {
            <an-view [style.padding]="'40'" [style.alignItems]="'center'" [style.gap]="'8'">
              <an-icon [name]="'search'" [size]="28" [color]="'#3a4356'" />
              <an-text [fontSize]="15" [color]="'#8a93a6'">Nothing saved matches that.</an-text>
            </an-view>
          }
        </an-scroll-view>

        <an-alert
          [visible]="open() !== null"
          [title]="open() ?? ''"
          [message]="'This is where the detail screen would open.'"
          [buttons]="['Close']"
          (select)="open.set(null)" />
      </an-safe-area>
    </an-view>
  `
})
export class AppComponent {
  protected readonly categories = CATEGORIES as unknown as string[]
  protected readonly String = String

  /** The scroll view's width in points, and the column width that falls out of
   *  it: two columns, a 12-point gutter, inside 16 points of padding a side. */
  private readonly viewportWidth = signal(0)
  protected readonly viewport = computed(() => String(this.viewportWidth()))
  protected readonly columnWidth = computed(() =>
    String(Math.max(0, Math.floor((this.viewportWidth() - 32 - 12) / 2)))
  )

  protected readonly query = signal('')
  protected readonly category = signal(0)
  protected readonly refreshing = signal(false)
  protected readonly open = signal<string | null>(null)

  protected readonly visible = computed(() => {
    const needle = this.query().trim().toLowerCase()
    const wanted = this.categories[this.category()]
    return ITEMS.filter((item) => {
      if (wanted !== 'All' && item.category !== wanted) return false
      if (!needle) return true
      return (
        item.title.toLowerCase().includes(needle) ||
        item.author.toLowerCase().includes(needle)
      )
    })
  })

  /** Dealt out between the columns by running height, not alternately: with two
   *  fixed lanes a run of tall cards leaves one side hanging. */
  protected readonly columns = computed(() => {
    const lanes: Item[][] = [[], []]
    const filled = [0, 0]
    for (const item of this.visible()) {
      const lane = filled[0] <= filled[1] ? 0 : 1
      lanes[lane].push(item)
      filled[lane] += item.height + 70
    }
    return lanes
  })

  protected onViewportLayout(event: NativeLayoutEvent): void {
    this.viewportWidth.set(event.width)
  }

  protected photo(id: number): string {
    return `https://picsum.photos/seed/an-feed-${id}/600/800`
  }

  protected onRefresh(): void {
    this.refreshing.set(true)
    setTimeout(() => this.refreshing.set(false), 1200)
  }
}
