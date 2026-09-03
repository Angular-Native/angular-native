import { ChangeDetectionStrategy, Component, signal } from '@angular/core'
import { NATIVE_PRIMITIVES, SafeArea } from '@angular-native/primitives'

/**
 * Wear OS with the same primitives as everywhere else.
 *
 * Unlike the Apple watch, there is nothing watch-shaped in the code here: a
 * Wear OS device runs `android.view.View`, so the host is the same one the
 * phone uses and these tags are the same tags. What changes is the screen, and
 * it changes in two ways the template can actually see:
 *
 * 1. **It is round.** Whatever is put in a corner is not clipped: it is never
 *    drawn, because there is no screen there. What moves it out of the way is
 *    `an-safe-area`, the same way it moves things out of the notch on a phone;
 *    the host counts the square inscribed in the circle as one more system
 *    inset. The template does not know whether the inset comes from a notch or
 *    from a curve, and it has no reason to.
 *
 * 2. **It is scrolled with the crown.** That is not declared: `an-scroll-view`
 *    listens to it on the watch, and `(scroll)` fires just as if a finger had
 *    dragged it. And if the raw crown is wanted as well —to raise a value, not
 *    to scroll—, `(crown)` delivers it in detents without the list stopping.
 *
 * The measurements are watch-face ones. The emulator screen is 227 points, and
 * the square that fits inside it is 160: a `fontSize` of 28 does not fit.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, SafeArea],
  template: `
    <!--
      The safe area wraps the scroll view and not the other way round. The other
      way round the content would reach the edge, which is the pretty thing on a
      watch, but the first and last rows would be eaten by the arc: on a round
      face, whatever sits right at the top sits at the narrowest point.
    -->
    <an-view
      [style.width]="'100%'"
      [style.flexGrow]="'1'"
      [backgroundColor]="'#0b1020'">
      <an-safe-area [style.width]="'100%'">
        <an-scroll-view
        [style.width]="'100%'"
        [style.flexGrow]="'1'"
        (scroll)="offset.set(Math.round($event.y))"
        (crown)="turn($event)"
        (crownIdle)="turning.set(false)">
        <an-view [style.width]="'100%'" [style.gap]="'6'" [style.paddingBottom]="'8'">
          <an-text [fontSize]="16" [fontWeight]="'bold'" [color]="'#f4f7ff'">
            angular-native
          </an-text>

          <an-text [fontSize]="11" [color]="'#9fb0d4'">
            Turn the crown: {{ offset() }} pt.
          </an-text>

          <an-text [fontSize]="11" [color]="turning() ? '#f59e0b' : '#64748b'">
            {{ detents().toFixed(1) }} detents{{ turning() ? ' · turning' : '' }}
          </an-text>

          @for (row of rows; track row.number) {
            <an-view
              [style.width]="'100%'"
              [style.height]="'30'"
              [borderRadius]="8"
              [backgroundColor]="row.number === marked() ? '#2b1e4a' : '#152036'"
              (press)="marked.set(row.number)">
              <an-text
                [style.width]="'100%'"
                [style.height]="'30'"
                [fontSize]="12"
                [textAlign]="'center'"
                [color]="row.number === marked() ? '#f59e0b' : '#9fb0d4'">
                {{ row.text }}
              </an-text>
            </an-view>
          }
        </an-view>
        </an-scroll-view>
      </an-safe-area>
    </an-view>
  `
})
export class AppComponent {
  /** The template cannot see globals; the rounding is done through this one. */
  protected readonly Math = Math

  /** How far the list has scrolled, in points. `(scroll)` keeps the count. */
  readonly offset = signal(0)

  /** The row that was tapped, to show that a finger still works too. */
  readonly marked = signal(0)

  /**
   * How far the crown has turned, in detents.
   *
   * This is not the same as `offset()`, which is how many points the list has
   * scrolled: one detent is about forty points, and whoever wants the crown for
   * something else —a volume, a time— wants the detent and not the scroll the
   * system did with it.
   */
  readonly detents = signal(0)

  /** Whether it is turning right now. `(crownIdle)` closes it. */
  readonly turning = signal(false)

  /**
   * The type is written out here rather than imported: `NativeCrownEvent`
   * exists in `packages/primitives` but its `public-api` does not export it
   * yet, so it cannot be asked for by name. The shape is the same, which is
   * what TypeScript checks, and the template already types it on its own
   * because the output is declared.
   */
  turn(event: { delta: number; offset: number; velocity: number }): void {
    this.detents.set(event.offset)
    this.turning.set(true)
  }

  readonly rows = [
    { number: 1, text: 'one' },
    { number: 2, text: 'two' },
    { number: 3, text: 'three' },
    { number: 4, text: 'four' },
    { number: 5, text: 'five' },
    { number: 6, text: 'six' },
    { number: 7, text: 'seven' },
    { number: 8, text: 'eight' },
    { number: 9, text: 'nine' },
    { number: 10, text: 'ten' },
    { number: 11, text: 'eleven' },
    { number: 12, text: 'twelve' }
  ]
}
