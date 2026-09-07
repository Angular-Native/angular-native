import { ChangeDetectionStrategy, Component, computed, signal } from '@angular/core'
import {
  NATIVE_PRIMITIVES,
  SafeArea,
  type NativeLayoutEvent,
  type NativePanEvent
} from '@angular-native/primitives'

/** One panel of the carousel.
 *
 *  The photographs come over the network from Lorem Picsum, with a fixed seed
 *  each so the same three pictures come back every run — a screenshot that
 *  changes every time it is taken is no use for comparing anything. An app of
 *  your own would point `source` at a bundled asset or at your own CDN; both
 *  go in the same prop. */
interface Panel {
  readonly photo: string
  readonly tint: string
  readonly caption: string
}

const PANELS: readonly Panel[] = [
  { photo: 'https://picsum.photos/seed/an-jacket-front/900/1100', tint: '#243b6b', caption: 'Front' },
  { photo: 'https://picsum.photos/seed/an-jacket-side/900/1100', tint: '#3b2452', caption: 'Side' },
  { photo: 'https://picsum.photos/seed/an-jacket-folded/900/1100', tint: '#123f3a', caption: 'Folded' }
]

const SIZES = ['S', 'M', 'L', 'XL'] as const

/**
 * A product page.
 *
 * It exists to be looked at — in a screenshot, in a video, in a simulator — so
 * every control on it is a real one: the segmented control is a
 * `UISegmentedControl`, the stepper is a `UIStepper`, the switch is a
 * `UISwitch`, and the stars are SF Symbols.
 *
 * The carousel is the one thing that is not a control, because no platform
 * ships one, and it is built the way this framework says to build that kind of
 * thing: a pan gesture moves a `translateX`, and letting go turns `[animate]`
 * on so the platform finishes the movement on its own drawing thread. Nothing
 * about it goes back through JavaScript frame by frame.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, SafeArea],
  template: `
    <an-view [style.flexGrow]="'1'" [backgroundColor]="'#0b0e14'">
    <an-safe-area [edges]="['top', 'bottom']" [style.flexGrow]="'1'">
      <an-navigation-bar
        [title]="'Field Jacket'"
        [showsBack]="true"
        [backTitle]="'Shop'"
        [style.height]="'44'" />

      <an-scroll-view
        [style.flexGrow]="'1'"
        [style.minHeight]="'0'"
        [style.overflow]="'scroll'">
        <!-- The carousel. The container reports its width and the panels are
             sized from it: there is no viewport unit here, and the same code
             works on a phone, a tablet and a Mac window being dragged. -->
        <an-view
          [style.height]="'320'"
          [style.marginHorizontal]="'16'"
          [borderRadius]="18"
          (layout)="onCarouselLayout($event)">
          <!-- The row is as wide as all three panels together, and that is not
               cosmetic: a touch is only delivered to a view if it falls inside
               that view's bounds, and the panels are children of this row. Left
               at the container's width, panels two and three sit outside their
               own parent, so once the carousel has moved on the finger is over
               a region no view will claim and the pan gesture is never seen
               again — the carousel advances once and then stops. -->
          <an-view
            [style.flexDirection]="'row'"
            [style.width]="trackWidth()"
            [style.height]="'320'"
            [translateX]="offset()"
            [animate]="settling() ? 260 : null"
            (pan)="onPan($event)">
            @for (panel of panels; track panel.caption) {
              <an-view
                [style.width]="width()"
                [style.height]="'320'"
                [backgroundColor]="panel.tint"
                [borderRadius]="18">
                <!-- The tint underneath is not decoration: it is what fills the
                     panel until the bytes land, so the carousel does not flash
                     white on a slow connection. -->
                <an-image
                  [source]="panel.photo"
                  [resizeMode]="'cover'"
                  [style.width]="width()"
                  [style.height]="'320'" />
                <an-view
                  [style.position]="'absolute'"
                  [style.left]="'20'"
                  [style.bottom]="'18'"
                  [backgroundColor]="'#0b0e14cc'"
                  [borderRadius]="6"
                  [style.paddingHorizontal]="'10'"
                  [style.paddingVertical]="'5'">
                  <an-text [fontSize]="12" [color]="'#f4f7ff'">{{ panel.caption }}</an-text>
                </an-view>
              </an-view>
            }
          </an-view>
        </an-view>

        <an-view
          [style.flexDirection]="'row'"
          [style.justifyContent]="'center'"
          [style.gap]="'6'"
          [style.paddingVertical]="'14'">
          @for (panel of panels; track panel.caption; let i = $index) {
            <an-view
              [style.width]="i === page() ? '18' : '6'"
              [style.height]="'6'"
              [borderRadius]="3"
              [animate]="200"
              [backgroundColor]="i === page() ? '#ff5a1f' : '#2a3242'"></an-view>
          }
        </an-view>

        <an-view [style.padding]="'20'" [style.gap]="'6'">
          <an-text [fontSize]="13" [color]="'#8a93a6'">WAXED COTTON</an-text>
          <an-text [fontSize]="26" [fontWeight]="'700'" [color]="'#f4f7ff'">Field Jacket</an-text>

          <an-view [style.flexDirection]="'row'" [style.alignItems]="'center'" [style.gap]="'6'"
                   [style.marginTop]="'2'">
            @for (star of stars; track star) {
              <an-icon [name]="'star'" [size]="14" [color]="'#ffb020'" />
            }
            <an-text [fontSize]="13" [color]="'#8a93a6'" [style.marginLeft]="'4'">
              4.8 · 212 reviews
            </an-text>
          </an-view>

          <an-text [fontSize]="22" [fontWeight]="'600'" [color]="'#f4f7ff'"
                   [style.marginTop]="'10'">{{ total() }}</an-text>
        </an-view>

        <an-view [style.paddingHorizontal]="'20'" [style.gap]="'18'">
          <an-view [style.gap]="'8'">
            <an-text [fontSize]="13" [color]="'#8a93a6'">Size</an-text>
            <an-segmented-control
              [items]="sizes"
              [selectedIndex]="size()"
              (change)="size.set($event.index)" />
          </an-view>

          <an-view [style.flexDirection]="'row'" [style.alignItems]="'center'"
                   [style.justifyContent]="'space-between'">
            <an-text [fontSize]="15" [color]="'#f4f7ff'">Quantity</an-text>
            <an-view [style.flexDirection]="'row'" [style.alignItems]="'center'" [style.gap]="'12'">
              <an-text [fontSize]="15" [color]="'#8a93a6'">{{ quantity() }}</an-text>
              <an-stepper
                [value]="quantity()"
                [minimumValue]="1"
                [maximumValue]="8"
                [step]="1"
                (change)="quantity.set($event.value)" />
            </an-view>
          </an-view>

          <an-view [style.flexDirection]="'row'" [style.alignItems]="'center'"
                   [style.justifyContent]="'space-between'">
            <an-text [fontSize]="15" [color]="'#f4f7ff'">Gift wrap</an-text>
            <an-switch [on]="gift()" (onChange)="gift.set($event)" />
          </an-view>
        </an-view>

        <an-view [style.padding]="'20'" [style.paddingTop]="'24'" [style.paddingBottom]="'32'">
          <an-text [fontSize]="14" [lineHeight]="21" [color]="'#8a93a6'">
            Twelve-ounce waxed cotton, cut long. Every control on this screen is
            the system's own — the segmented control, the stepper, the switch
            and the stars are UIKit and Material, not drawings of them. The
            carousel is not a control anywhere, so it is a pan gesture moving a
            transform, which the platform finishes on its own drawing thread.
          </an-text>
        </an-view>
      </an-scroll-view>

      <an-view [style.padding]="'16'" [style.paddingTop]="'12'">
        <an-button
          [title]="'Add to bag · ' + total()"
          [variant]="'filled'"
          [color]="'#ff5a1f'"
          [style.height]="'50'"
          (press)="added.set(true)" />
      </an-view>

      <an-alert
        [visible]="added()"
        [title]="'Added to bag'"
        [message]="quantity() + ' × Field Jacket, size ' + sizes[size()]"
        [buttons]="['Keep shopping']"
        (select)="added.set(false)" />
    </an-safe-area>
    </an-view>
  `
})
export class AppComponent {
  protected readonly panels = PANELS
  protected readonly sizes = SIZES as unknown as string[]
  protected readonly stars = [0, 1, 2, 3, 4]

  protected readonly size = signal(1)
  protected readonly quantity = signal(1)
  protected readonly gift = signal(false)
  protected readonly added = signal(false)

  /** The carousel's page width, straight from the frame the core resolved. */
  protected readonly width = signal('0')
  private readonly pageWidth = signal(0)
  protected readonly page = signal(0)
  protected readonly settling = signal(true)
  private readonly drag = signal(0)

  protected readonly offset = computed(
    () => -this.page() * this.pageWidth() + this.drag()
  )

  /** Every panel side by side, which is what the row has to be able to hold. */
  protected readonly trackWidth = computed(() =>
    String(this.pageWidth() * this.panels.length)
  )

  protected readonly total = computed(() => {
    const price = 248 * this.quantity() + (this.gift() ? 6 : 0)
    return '$' + price.toLocaleString('en-US')
  })

  protected onCarouselLayout(event: NativeLayoutEvent): void {
    this.pageWidth.set(event.width)
    this.width.set(String(event.width))
  }

  protected onPan(event: NativePanEvent): void {
    if (event.state === 'move') {
      this.settling.set(false)
      this.drag.set(event.translationX)
      return
    }
    // Let go: decide which page won, then hand the rest to the platform.
    const width = this.pageWidth()
    const moved = event.state === 'cancel' ? 0 : event.translationX
    const threshold = width * 0.22
    let next = this.page()
    if (moved < -threshold) next = Math.min(this.page() + 1, this.panels.length - 1)
    if (moved > threshold) next = Math.max(this.page() - 1, 0)
    this.settling.set(true)
    this.drag.set(0)
    this.page.set(next)
  }
}
