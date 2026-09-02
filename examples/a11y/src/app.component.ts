import { ChangeDetectionStrategy, Component } from '@angular/core'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'

/**
 * What a screen reader is told about this screen.
 *
 * Every row here exists to be *read*, not to be looked at, and the check that
 * goes with it —`scripts/check-accessibility.sh`— does not look at it either:
 * it launches the app and walks the accessibility tree from outside the
 * process, through the same API VoiceOver uses. So what is written in this
 * template is what has to come back out of that walk.
 *
 * ## Why `[attr.…]` and not `[accessibilityLabel]`
 *
 * The contract declares the six props as typed inputs on `NativeVisual`, and
 * that is where they should be written from. They do not travel yet: the
 * directive only pushes to the core what its constructor lists in `push({…})`,
 * and the six are not in that list. Adding them is one line each in
 * `packages/primitives/src/primitives.ts`, which this branch deliberately does
 * not touch.
 *
 * `[attr.…]` reaches the same place by the other door —`Renderer2.setAttribute`
 * also lands on `dom.setProp`— so the hosts receive exactly the same props
 * with exactly the same names, and everything below the renderer is being
 * exercised for real. What is lost is the typing, which is the whole point of
 * the inputs: a typo here compiles. That is the reason this is a workaround
 * and not the way it should stay.
 *
 * ## What every block is for
 *
 * The screen is not a gallery of controls: each block is a case the hosts had
 * to decide something about, and the decision is in the comment.
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
      [style.gap]="'14'"
      [backgroundColor]="'#0b1020'">
      <!-- A heading. The one role that is a heading in all three: .header in
           UIKit, AXHeading in AppKit, .isHeader in SwiftUI. -->
      <an-text
        [fontSize]="24"
        [fontWeight]="700"
        [color]="'#f8fafc'"
        [attr.accessibilityRole]="'header'">
        Accessibility
      </an-text>

      <!-- A plain view doing a button's job. Three things at once: it gets a
           name it did not have, it gets told what happens when you activate
           it, and 'accessible' turns the row —icon plus label— into a single
           stop instead of two. -->
      <an-view
        [style.flexDirection]="'row'"
        [style.alignItems]="'center'"
        [style.gap]="'8'"
        [style.height]="44"
        [style.paddingHorizontal]="'12'"
        [borderRadius]="10"
        [backgroundColor]="'#1e3a5f'"
        [attr.accessible]="'true'"
        [attr.accessibilityRole]="'button'"
        [attr.accessibilityLabel]="'Play'"
        [attr.accessibilityHint]="'Starts the track from the beginning'">
        <an-icon [name]="'favorite'" [size]="16" [color]="'#93c5fd'" />
        <an-text [fontSize]="15" [color]="'#e2e8f0'">Play</an-text>
      </an-view>

      <!-- A system control with nothing on top. This is the one that has to
           come out of the walk with AppKit's own label —its title— and
           AppKit's own role. If a host wrote an empty label over it, this row
           would go nameless, and that is exactly what the check watches. -->
      <an-button
        [style.height]="36"
        [title]="'Untouched button'"
        [variant]="'tonal'"
        [color]="'#93c5fd'"></an-button>

      <!-- The same control with a label of ours. Here the template did say
           something, so ours wins over the title: the button reads "Save the
           changes you made" and not "Save". -->
      <an-button
        [style.height]="36"
        [title]="'Save'"
        [variant]="'tonal'"
        [color]="'#93c5fd'"
        [attr.accessibilityLabel]="'Save the changes you made'"></an-button>

      <!-- A switch, on. 'checked' is nobody's trait: it goes into the value,
           and it goes in with the platform's own convention rather than a word
           of ours, so the reader says it in the user's language. -->
      <an-switch
        [on]="true"
        [color]="'#34d399'"
        [attr.accessibilityLabel]="'Night mode'"
        [attr.accessibilityRole]="'switch'"
        [attr.accessibilityState]="'{&quot;checked&quot;:true}'"></an-switch>

      <!-- Something that behaves like a slider without being one. The value is
           the template's, so nothing derived may overwrite it. -->
      <an-view
        [style.height]="30"
        [borderRadius]="6"
        [backgroundColor]="'#1e293b'"
        [attr.accessibilityRole]="'slider'"
        [attr.accessibilityLabel]="'Volume'"
        [attr.accessibilityValue]="'60 per cent'"></an-view>

      <!-- Selected and disabled at once. In UIKit these two are the only bits
           of state that live in the trait mask; in AppKit they are two
           separate properties. -->
      <an-view
        [style.height]="30"
        [borderRadius]="6"
        [backgroundColor]="'#312e42'"
        [attr.accessibilityRole]="'button'"
        [attr.accessibilityLabel]="'Chosen and switched off'"
        [attr.accessibilityState]="'{&quot;selected&quot;:true,&quot;disabled&quot;:true}'"></an-view>

      <!-- Purely decorative. 'accessible="false"' has to take the text inside
           with it: hiding only the parent hides nothing, because the child
           would still be a stop of its own. -->
      <an-view
        [style.height]="24"
        [borderRadius]="4"
        [backgroundColor]="'#172033'"
        [attr.accessible]="'false'">
        <an-text [fontSize]="11" [color]="'#334155'">decorative filler</an-text>
      </an-view>

      <!-- And the three that no Apple platform can express, one per platform,
           so that "it is said out loud" is something a check can read in the
           log rather than something a comment claims.

           'radio'   has no UIKit trait.
           'summary' has no AppKit role.
           'busy'    has no shape anywhere. -->
      <an-view
        [style.height]="20"
        [attr.accessibilityRole]="'radio'"
        [attr.accessibilityLabel]="'No trait in UIKit'"></an-view>
      <an-view
        [style.height]="20"
        [attr.accessibilityRole]="'summary'"
        [attr.accessibilityLabel]="'No role in AppKit'"></an-view>
      <an-view
        [style.height]="20"
        [attr.accessibilityRole]="'button'"
        [attr.accessibilityLabel]="'Busy fits nowhere'"
        [attr.accessibilityState]="'{&quot;busy&quot;:true}'"></an-view>
    </an-view>
  `
})
export class AppComponent {}
