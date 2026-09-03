import { ChangeDetectionStrategy, Component, signal } from '@angular/core'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'
import type { NativeCursor, NativeHoverEvent } from '@angular-native/primitives'

/**
 * What only exists on a desktop: the pointer, the shape of the pointer, and the
 * trackpad swipe gesture.
 *
 * There is none of the three on a phone, so this example is not meant to be
 * looked at in the simulator: it is meant to be looked at on a Mac, with a mouse
 * on it. All three things are delivered by the system —`NSTrackingArea`,
 * `NSCursor` and `swipeWithEvent:`— and none of them is drawn here.
 *
 * The navigation bar at the top is deliberately not visible in the content: on
 * macOS the `[title]` of an `<an-navigation-bar>` ends up in the window's title
 * bar, which is where a Mac user looks for it, and the node takes up no room.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-view
      [style.width]="'100%'"
      [style.height]="'100%'"
      [style.padding]="'20'"
      [style.gap]="'18'"
      [backgroundColor]="'#0b1020'">
      <an-navigation-bar [title]="title()" />

      <an-text [fontSize]="26" [fontWeight]="700" [color]="'#f8fafc'">desktop</an-text>
      <an-text [fontSize]="13" [color]="'#94a3b8'">{{ hint() }}</an-text>

      <!-- Hovering. The background, the border and the label change with
           (hover), and the pointer changes with [cursor]. -->
      <an-text [fontSize]="15" [fontWeight]="600" [color]="'#cbd5e1'">hover</an-text>
      <an-view [style.flexDirection]="'row'" [style.gap]="'12'" [style.height]="86">
        @for (card of cards; track card.name) {
          <an-view
            [style.flexGrow]="'1'"
            [style.alignItems]="'center'"
            [style.justifyContent]="'center'"
            [style.gap]="'6'"
            [borderRadius]="12"
            [borderWidth]="2"
            [borderColor]="hovered() === card.name ? '#6ee7b7' : '#1e293b'"
            [backgroundColor]="hovered() === card.name ? '#12324a' : '#111a2e'"
            [cursor]="card.cursor"
            (hover)="onHover(card.name, $event)">
            <an-text [fontSize]="15" [fontWeight]="600" [color]="'#e2e8f0'">
              {{ card.name }}
            </an-text>
            <an-text [fontSize]="12" [color]="'#94a3b8'">
              {{ hovered() === card.name ? 'inside' : 'outside' }}
            </an-text>
          </an-view>
        }
      </an-view>

      <!-- A system control can report the pointer being over it too: the
           tracking area is not carried by the view, it is carried by a separate
           object, so there is no need to subclass the NSButton. -->
      <an-button
        [style.height]="40"
        [title]="buttonHovered() ? 'and a system button too' : 'come over here'"
        [variant]="'tonal'"
        [color]="'#6ee7b7'"
        [cursor]="'pointer'"
        (hover)="buttonHovered.set($event.hovered)"
        (press)="presses.set(presses() + 1)"></an-button>

      <!-- Swiping. Two fingers on the trackpad, with the system threshold. -->
      <an-text [fontSize]="15" [fontWeight]="600" [color]="'#cbd5e1'">swipe</an-text>
      <an-view
        [style.flex]="1"
        [style.alignItems]="'center'"
        [style.justifyContent]="'center'"
        [style.gap]="'8'"
        [borderRadius]="14"
        [backgroundColor]="'#111a2e'"
        [cursor]="'grab'"
        (swipeLeft)="onSwipe('left', '←')"
        (swipeRight)="onSwipe('right', '→')"
        (swipeUp)="onSwipe('up', '↑')"
        (swipeDown)="onSwipe('down', '↓')">
        <an-text [fontSize]="34" [fontWeight]="700" [color]="'#6ee7b7'">{{ arrow() }}</an-text>
        <an-text [fontSize]="14" [color]="'#e2e8f0'">{{ lastSwipe() }}</an-text>
        <an-text [fontSize]="12" [color]="'#94a3b8'">
          swipes: {{ swipes() }} · presses: {{ presses() }}
        </an-text>
      </an-view>
    </an-view>
  `
})
export class AppComponent {
  /**
   * Each card shows a different system pointer. They are `NSCursor`'s own,
   * asked for by their CSS name.
   */
  readonly cards: ReadonlyArray<{ name: string; cursor: NativeCursor }> = [
    { name: 'pointer', cursor: 'pointer' },
    { name: 'text', cursor: 'text' },
    { name: 'crosshair', cursor: 'crosshair' },
    { name: 'not-allowed', cursor: 'not-allowed' }
  ]

  readonly hovered = signal<string | null>(null)
  readonly buttonHovered = signal(false)
  readonly presses = signal(0)
  readonly swipes = signal(0)
  readonly lastSwipe = signal('nothing yet')

  readonly title = signal('desktop · angular-native')

  readonly hint = signal(
    'hover the cards with the mouse and swipe with two fingers in the box below'
  )

  readonly arrow = signal('·')

  onHover(name: string, event: NativeHoverEvent): void {
    this.hovered.set(event.hovered ? name : null)
    if (event.hovered) {
      // The point arrives in view coordinates, just like a (press) one.
      console.log(`[hover] inside ${name} at ${Math.round(event.x)},${Math.round(event.y)}`)
    }
  }

  onSwipe(direction: string, arrow: string): void {
    this.swipes.set(this.swipes() + 1)
    this.lastSwipe.set(direction)
    this.arrow.set(arrow)
    console.log(`[swipe] ${direction}`)
  }
}
