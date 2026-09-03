import { ChangeDetectionStrategy, Component, signal } from '@angular/core'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'

/**
 * The same kind of component that runs on the phone, in a volumetric window.
 *
 * Two things this screen shows on purpose:
 *
 * 1. **The background is not painted whole.** The visionOS window already brings
 *    one: the glass the system draws, with its blur and its shadow over the real
 *    room. The shell leaves the root transparent and only the cards are painted
 *    here, so the glass shows between them. A `[backgroundColor]` on the top
 *    container would cover it and the app would be an opaque slab floating in
 *    the living room.
 *
 * 2. **There is no screen size.** The user pulls the corner and the window
 *    changes size whenever they like. Nothing here is in fixed points: the
 *    widths go in percentages and in `flexGrow`, and the viewport arrives
 *    through `viewDidLayoutSubviews` as in any other family. The `an-view`s with
 *    `(press)` are highlighted by the system when looked at, because the host
 *    gives them a `hoverStyle`.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-view
      [style.width]="'100%'"
      [style.height]="'100%'"
      [style.padding]="'40'"
      [style.gap]="'24'">

      <an-text [fontSize]="44" [fontWeight]="'bold'" [color]="'#f4f7ff'">angular-native</an-text>

      <an-text [fontSize]="20" [color]="'#c8d3ea'">
        No background of its own: what shows behind is the window's glass, which
        the system paints. Look at a card and pinch it.
      </an-text>

      <an-view [style.flexDirection]="'row'" [style.gap]="'24'" [style.width]="'100%'">
        @for (card of cards; track card) {
          <an-view
            [style.flexGrow]="'1'"
            [style.height]="'160'"
            [borderRadius]="24"
            [backgroundColor]="'#1e2a4a'"
            (press)="choose(card)">
            <an-text
              [style.width]="'100%'"
              [style.height]="'160'"
              [fontSize]="24"
              [textAlign]="'center'"
              [color]="'#f4f7ff'">{{ card }}</an-text>
          </an-view>
        }
      </an-view>

      <an-text [fontSize]="20" [color]="'#9fb0d4'">{{ chosen() }}</an-text>
    </an-view>
  `
})
export class AppComponent {
  readonly cards = ['one', 'two', 'three']
  readonly chosen = signal('nothing chosen yet')

  choose(card: string): void {
    this.chosen.set(`you chose: ${card}`)
  }
}
