import { ChangeDetectionStrategy, Component, computed, signal } from '@angular/core'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'

/**
 * The same kind of component that runs on the phone, with the subset of
 * primitives the watch knows how to paint today: `View`, `Text`, `Button` and
 * `ScrollView`.
 *
 * The measurements are watch ones and not phone ones. On a screen 176 points
 * wide a `fontSize` of 28 eats half the view, so the heading is 18 and the body
 * 13. The `paddingTop` is not the iPhone's either: there is no notch here, but
 * there is the system clock painted over the app in the top corner, and room has
 * to be left for it.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <!--
      The height goes through \`flexGrow\` and not through \`height: 100%\`: the core
      gives everything scrollable \`flex-basis: 0\` so it fits inside its parent,
      and on the main axis the basis beats the height. With a plain \`height\` the
      ScrollView measures zero and nothing is visible.
    -->
    <an-scroll-view [style.width]="'100%'" [style.flexGrow]="'1'" [backgroundColor]="'#0b1020'">
      <an-view
        [style.paddingTop]="'44'"
        [style.paddingHorizontal]="'10'"
        [style.paddingBottom]="'16'"
        [style.gap]="'8'"
        [style.width]="'100%'">

        <an-text [fontSize]="18" [fontWeight]="'bold'" [color]="'#f4f7ff'">angular-native</an-text>

        <an-text [fontSize]="13" [color]="'#9fb0d4'">
          Angular with signals, on QuickJS, with taffy's layout. Here it is
          painted by SwiftUI because the watch has no UIView.
        </an-text>

        <an-view
          [style.height]="'44'"
          [style.width]="'100%'"
          [borderRadius]="10"
          [backgroundColor]="'#1e2a4a'"
          (press)="tap()">
          <an-text
            [style.width]="'100%'"
            [style.height]="'44'"
            [fontSize]="15"
            [textAlign]="'center'"
            [color]="'#6ee7b7'">{{ label() }}</an-text>
        </an-view>

        <an-button
          [title]="'reset'"
          [color]="'#f59e0b'"
          [backgroundColor]="'#2b1e4a'"
          [borderRadius]="10"
          (press)="taps.set(0)"></an-button>

        <!-- So there is something to scroll and the crown can be seen working. -->
        @for (row of rows; track row) {
          <an-view [style.height]="'26'" [borderRadius]="6" [backgroundColor]="'#152036'">
            <an-text
              [style.width]="'100%'"
              [style.height]="'26'"
              [fontSize]="12"
              [textAlign]="'center'"
              [color]="'#9fb0d4'">{{ row }}</an-text>
          </an-view>
        }
      </an-view>
    </an-scroll-view>
  `
})
export class AppComponent {
  readonly taps = signal(0)
  readonly seconds = signal(0)

  readonly label = computed(() =>
    this.taps() === 0 ? 'tap here' : `taps: ${this.taps()}`
  )

  readonly rows = ['one', 'two', 'three', 'four', 'five', 'six']

  tap(): void {
    this.taps.update((value) => value + 1)
  }

  constructor() {
    // The JS clock is driven by the frame, not by a separate thread: this
    // advances with the shell's timer, just as on iOS it advances with the
    // CADisplayLink.
    setInterval(() => this.seconds.update((value) => value + 1), 1000)
  }
}
