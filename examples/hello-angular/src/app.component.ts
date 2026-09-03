import { ChangeDetectionStrategy, Component, computed, signal } from '@angular/core'
import { NATIVE_PRIMITIVES, type NativePressEvent } from '@angular-native/primitives'

/**
 * An ordinary Angular component. The only difference is that the elements are
 * native primitives: the template, the signals, the `@for` and the bindings are
 * exactly the usual ones.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-view [style.paddingTop]="'64'" [style.paddingHorizontal]="'16'" [style.gap]="'16'"
          [backgroundColor]="'#0b1020'" [style.width]="'100%'" [style.height]="'100%'">

      <an-text [fontSize]="28" [fontWeight]="'bold'" [color]="'#f4f7ff'">angular-native</an-text>

      <an-view [style.flexDirection]="'row'" [style.gap]="'12'">
        @for (card of cards; track card.color) {
          <an-view [style.flexGrow]="card.grow" [style.height]="'88'"
                [backgroundColor]="card.color"
                [borderTopLeftRadius]="card.corners[0]"
                [borderTopRightRadius]="card.corners[1]"
                [borderBottomRightRadius]="card.corners[2]"
                [borderBottomLeftRadius]="card.corners[3]"
                (press)="onPress($event)"
                (doublePress)="taps.set(0)"></an-view>
        }
      </an-view>

      <an-text [fontSize]="16" [color]="'#f4f7ff'">{{ tapLabel() }}</an-text>

      <an-text [fontSize]="16" [color]="'#9fb0d4'">
        This is an Angular template with signals, running on QuickJS.
        Every element is a native view: UIView on iOS, View on Android.
      </an-text>

      <an-text [fontSize]="16" [color]="'#6ee7b7'">{{ label() }}</an-text>

      @if (seconds() >= 3) {
        <an-text [fontSize]="14" [color]="'#f59e0b'">The &#64;if came in at 3 seconds.</an-text>
      }
    </an-view>
  `
})
export class AppComponent {
  readonly cards = [
    // A different radius per corner, which is what UIKit cannot do on its own.
    { color: '#1e2a4a', grow: 1, corners: [24, 4, 24, 4] },
    { color: '#2b1e4a', grow: 2, corners: [4, 24, 4, 24] }
  ]

  readonly seconds = signal(0)
  readonly label = computed(() => `seconds running: ${this.seconds()}`)

  readonly taps = signal(0)
  readonly lastPoint = signal<NativePressEvent | null>(null)
  readonly tapLabel = computed(() => {
    const point = this.lastPoint()
    if (!point) return 'tap a card; a double tap resets it'
    return `taps: ${this.taps()} (last at ${Math.round(point.x)}, ${Math.round(point.y)})`
  })

  onPress(event: NativePressEvent): void {
    this.taps.update((value) => value + 1)
    this.lastPoint.set(event)
  }

  constructor() {
    setInterval(() => this.seconds.update((value) => value + 1), 1000)
  }
}
