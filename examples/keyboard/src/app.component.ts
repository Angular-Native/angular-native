import { ChangeDetectionStrategy, Component, signal } from '@angular/core'
import {
  NATIVE_PRIMITIVES,
  SafeArea,
  NativeSafeAreaInsets
} from '@angular-native/primitives'

/**
 * A form with its last field pinned to the bottom of the screen — the one
 * layout every mobile framework has to get right and the one the keyboard
 * covers.
 *
 * Nothing here knows the keyboard exists. The whole form sits inside an
 * `<an-safe-area>` that asks to be kept clear of the bottom edge, which is the
 * same thing it asks in order to stay off the home indicator, and the host
 * answers with the keyboard folded into that inset. That is the point of the
 * example: **one question, and the template does not change** between a phone
 * with a notch, a watch with a round bezel and a phone with the keyboard up.
 *
 * The reported inset is written on screen because a check has to be able to
 * read it — see `scripts/check-keyboard-device.sh`, which taps the last field
 * and asks the system where the field ended up.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, SafeArea],
  template: `
    <an-view
      [style.width]="'100%'"
      [style.height]="'100%'"
      [backgroundColor]="'#0b1020'"
      (safeArea)="bottom.set(Math.round($event.bottom))">
      <an-safe-area [edges]="['top', 'bottom']" [padding]="16" [style.gap]="'12'">
        <an-text [fontSize]="24" [fontWeight]="'bold'" [color]="'#f4f7ff'">Sign in</an-text>

        <an-text [fontSize]="13" [color]="'#6ee7b7'" [accessibilityLabel]="'inset'">
          bottom inset {{ bottom() }}
        </an-text>

        <an-text-input
          [style.height]="'44'"
          [placeholder]="'email'"
          [placeholderColor]="'#6b7a99'"
          [value]="email()"
          [color]="'#f4f7ff'"
          [fontSize]="16"
          [keyboardType]="'email'"
          [autoCapitalize]="'none'"
          [autoCorrect]="false"
          [backgroundColor]="'#1e2a4a'"
          [borderRadius]="8"
          [accessibilityLabel]="'email'"
          (valueChange)="email.set($event)" />

        <!-- The spacer is what pushes the last field down. Without it the form
             would sit at the top and the bug would not show. -->
        <an-view [style.flexGrow]="'1'" />

        <an-text [fontSize]="13" [color]="'#9fb0d4'">Last field, at the very bottom:</an-text>

        <an-text-input
          [style.height]="'44'"
          [placeholder]="'password'"
          [placeholderColor]="'#6b7a99'"
          [value]="password()"
          [color]="'#f4f7ff'"
          [fontSize]="16"
          [secureTextEntry]="true"
          [autoCapitalize]="'none'"
          [autoCorrect]="false"
          [backgroundColor]="'#1e2a4a'"
          [borderRadius]="8"
          [accessibilityLabel]="'password'"
          (valueChange)="password.set($event)" />
      </an-safe-area>
    </an-view>
  `
})
export class AppComponent {
  protected readonly email = signal('')
  protected readonly password = signal('')

  /**
   * The bottom inset the host last reported, rounded. It is read off the same
   * event `an-safe-area` listens to; having it on screen is what lets a check
   * tell "the keyboard was never reported" from "it was reported and the
   * layout ignored it".
   */
  protected readonly bottom = signal(0)

  /** `Math` is not in a template's scope unless something puts it there. */
  protected readonly Math = Math
}
