import { ChangeDetectionStrategy, Component, signal } from '@angular/core'
import { NATIVE_PRIMITIVES, type NativeScrollEvent } from '@angular-native/primitives'

/** Enough cards that neither strip fits, whichever way it runs. */
const CARDS = Array.from({ length: 12 }, (_, index) => ({
  id: index,
  title: `Card ${index + 1}`,
  tint: ['#1d4ed8', '#0f766e', '#b45309', '#9333ea'][index % 4]
}))

/**
 * The two things a scroll view was getting wrong.
 *
 * The first strip is given a `[style.height]`, which for a long time resolved
 * to nothing: the core pinned `flex-basis: 0` on every scroll view so that its
 * content could not size it, and a non-`auto` basis beats `height` on the main
 * axis. The view came out zero points tall and no host, no log and no check
 * said a word.
 *
 * The second is `[horizontal]`, which did not exist: the content size was
 * clamped to the frame's width on the way out of layout, so a row of cards
 * wider than the screen arrived at the host cut back to the screen.
 *
 * The third is the ordinary vertical one, here so that the check has something
 * to compare against: whatever the two above do, this must go on behaving the
 * way it did.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-view [style.flexGrow]="'1'" [backgroundColor]="'#0b0e14'" [style.padding]="'16'"
             [style.gap]="'12'">
      <an-text [fontSize]="20" [fontWeight]="'700'" [color]="'#f4f7ff'">Scrolling</an-text>

      <!-- 160 points, and 160 points is what it has to come out at, however
           much taller its content is. -->
      <an-scroll-view
        [testID]="'tall'"
        [style.height]="'160'"
        [backgroundColor]="'#141926'">
        @for (card of cards; track card.id) {
          <an-view [style.height]="'60'" [backgroundColor]="card.tint" [style.margin]="'4'" />
        }
      </an-scroll-view>

      <!-- Sideways. The children run along x without the template saying so:
           a horizontal scroll view whose children stack downwards has nothing
           wider than itself and therefore nothing to scroll. -->
      <an-scroll-view
        [testID]="'sideways'"
        [horizontal]="true"
        [style.height]="'96'"
        (scroll)="offset.set($event)">
        @for (card of cards; track card.id) {
          <an-view
            [style.width]="'120'"
            [style.height]="'96'"
            [style.marginRight]="'8'"
            [backgroundColor]="card.tint"
            [style.padding]="'10'">
            <an-text [fontSize]="13" [color]="'#ffffff'">{{ card.title }}</an-text>
          </an-view>
        }
      </an-scroll-view>

      <!-- And the template still has the last word on the direction. -->
      <an-scroll-view
        [testID]="'sideways-but-stacked'"
        [horizontal]="true"
        [style.flexDirection]="'column'"
        [style.height]="'40'">
        @for (card of cards; track card.id) {
          <an-view [style.width]="'20'" [style.height]="'20'" [backgroundColor]="card.tint" />
        }
      </an-scroll-view>

      <an-scroll-view [testID]="'plain'" [style.flexGrow]="'1'" [style.minHeight]="'0'">
        @for (card of cards; track card.id) {
          <an-view [style.height]="'70'" [backgroundColor]="card.tint" [style.marginBottom]="'6'" />
        }
      </an-scroll-view>
    </an-view>
  `
})
export class AppComponent {
  protected readonly cards = CARDS
  protected readonly offset = signal<NativeScrollEvent | null>(null)
}
