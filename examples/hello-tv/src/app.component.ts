import { ChangeDetectionStrategy, Component, signal } from '@angular/core'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'

/**
 * The same kind of component that runs on the phone, with the measurements and
 * the interaction model of a television.
 *
 * Three things that are not cosmetic and that this screen shows on purpose:
 *
 * 1. **There are no taps.** Nothing here is touched: the remote moves the focus
 *    from one view to another and the centre button presses whichever is
 *    focused. A view that cannot take the focus cannot be pressed, so a
 *    `(press)` on anything that is not an `an-view` or a system control never
 *    fires. See `docs/tvos.md`.
 *
 * 2. **The margins are television ones.** The edges of a TV set are cropped
 *    —overscan—, and Apple asks for 90 points left and right and 60 top and
 *    bottom. The viewport is 1920x1080 points, not an iPhone's 393: a `fontSize`
 *    of 28 here cannot be read from the sofa.
 *
 * 3. **The focus highlight is painted by each control, not by the system.** The
 *    two `an-button`s are `UIButton`s and lift and turn white on their own when
 *    focused, because UIKit draws that: which is why there are two and not one.
 *    Moving the focus between them is visible. The `an-view` below is focusable
 *    —the host creates it as `AnFocusableView`— and pressable, but it is not
 *    highlighted: tvOS has no `UIFocusEffect`, and none is drawn here by hand.
 *    That the focus got there shows in its counter when the centre button is
 *    pressed.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-view
      [style.width]="'100%'"
      [style.height]="'100%'"
      [style.paddingHorizontal]="'90'"
      [style.paddingVertical]="'60'"
      [style.gap]="'28'"
      [backgroundColor]="'#0b1020'">

      <an-text [fontSize]="76" [fontWeight]="'bold'" [color]="'#f4f7ff'">angular-native</an-text>

      <an-text [fontSize]="30" [color]="'#9fb0d4'">
        Angular with signals, on QuickJS, with taffy's layout, over real UIViews.
        The same host as the phone; what changes is that here you navigate with
        the remote.
      </an-text>

      <!--
        Two system buttons, one below the other. They are UIButtons, so tvOS
        already knows how to focus them and it is tvOS that lifts them and turns
        them white when the focus arrives. They are here so that moving the focus
        with the remote shows in a screenshot without drawing anything.
      -->
      <an-button
        [title]="'top — pressed ' + top() + ' times'"
        [fontSize]="34"
        [color]="'#0b1020'"
        [backgroundColor]="'#6ee7b7'"
        [borderRadius]="16"
        [style.width]="'760'"
        [style.height]="'88'"
        (press)="top.set(top() + 1)"></an-button>

      <an-button
        [title]="'bottom — pressed ' + bottom() + ' times'"
        [fontSize]="34"
        [color]="'#0b1020'"
        [backgroundColor]="'#fca5a5'"
        [borderRadius]="16"
        [style.width]="'760'"
        [style.height]="'88'"
        (press)="bottom.set(bottom() + 1)"></an-button>

      <!--
        A bare view with (press). On iOS this is a UIView with a
        UITapGestureRecognizer and that is that. On tvOS it would also have to
        answer yes to canBecomeFocused, and a UIView answers no: the host creates
        it as AnFocusableView so the remote can stop here. Without that, this
        rectangle would be unreachable.
      -->
      <an-view
        [style.width]="'760'"
        [style.height]="'88'"
        [borderRadius]="16"
        [backgroundColor]="'#1e2a4a'"
        (press)="view.set(view() + 1)">
        <an-text
          [style.width]="'760'"
          [style.height]="'88'"
          [fontSize]="30"
          [textAlign]="'center'"
          [color]="'#f4f7ff'">an-view, not a button — pressed {{ view() }} times</an-text>
      </an-view>

      <an-text [fontSize]="26" [color]="'#5f7099'">
        you have been here {{ seconds() }} seconds
      </an-text>
    </an-view>
  `
})
export class AppComponent {
  readonly top = signal(0)
  readonly bottom = signal(0)
  /** The `an-view`. Its separate counter is what proves the focus reached a view
   *  that is not a control: had the button been pressed, the other one would go
   *  up. */
  readonly view = signal(0)
  readonly seconds = signal(0)

  constructor() {
    // The JS clock is driven by the frame, not by a separate thread: this
    // advances with the shell's CADisplayLink, just as on the phone.
    setInterval(() => this.seconds.update((value) => value + 1), 1000)
  }
}
