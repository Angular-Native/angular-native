import { ChangeDetectionStrategy, Component, signal } from '@angular/core'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'
import type { NativeAccessibilityState } from '@angular-native/primitives'


/**
 * A screen whose only job is to say things that can only be seen with a screen
 * reader.
 *
 * It is not meant to be looked at: it is meant to be dumped. Every row is one
 * case of the accessibility contract, and `scripts/check-a11y-device.sh`
 * installs it, asks the system for the tree with `uiautomator dump` and checks
 * that what the system says about each node is what this template asked for.
 * Hence the very literal labels: they are the key the dump finds each row by.
 *
 * The last three rows are the ones nobody thinks of: an `an-switch` and an
 * `an-button` without a single accessibility prop — which have to keep
 * announcing themselves the way Material announces them — and a hidden row,
 * which has to disappear from the tree along with what it carries inside.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-view [style.padding]="'6'" [style.gap]="'2'" [backgroundColor]="'#0b1020'">
      <!-- A heading. On Android it is not a class, it is a flag. -->
      <an-text
        [fontSize]="13"
        [color]="'#f4f7ff'"
        [accessibilityRole]="'header'"
        [accessibilityLabel]="'Settings'">
        Settings
      </an-text>

      <!--
        An 'an-view' acting as a button. This is the case that justifies all of
        it: with no role and no name it is a rectangle you can press and
        nothing can be known about.
      -->
      <an-view
        [style.height]="'28'"
        [borderRadius]="6"
        [backgroundColor]="'#1e2a4a'"
        [accessibilityRole]="'button'"
        [accessibilityLabel]="'Save the draft'"
        [accessibilityHint]="'saves it without leaving the screen'"
        (press)="saved.set(saved() + 1)">
        <an-text [fontSize]="10" [color]="'#9fb0d4'">Save ({{ saved() }})</an-text>
      </an-view>

      <!-- A link: Android has no widget, so it goes through roleDescription. -->
      <an-view
        [style.height]="'26'"
        [accessibilityRole]="'link'"
        [accessibilityLabel]="'Open the website'"
        (press)="touch()">
        <an-text [fontSize]="10" [color]="'#7aa2f7'">Open the website</an-text>
      </an-view>

      <!-- The third state of a checkbox, which does not exist on Android. -->
      <an-view
        [style.height]="'26'"
        [accessibilityRole]="'checkbox'"
        [accessibilityLabel]="'Select all'"
        [accessibilityState]="mixed"
        (press)="touch()">
        <an-text [fontSize]="10" [color]="'#9fb0d4'">Select all</an-text>
      </an-view>

      <!-- And the one that does exist, to compare against. -->
      <an-view
        [style.height]="'26'"
        [accessibilityRole]="'checkbox'"
        [accessibilityLabel]="'Alerts'"
        [accessibilityState]="checked"
        (press)="touch()">
        <an-text [fontSize]="10" [color]="'#9fb0d4'">Alerts</an-text>
      </an-view>

      <!--
        Value and state together. 'disabled' turns the node off but not the
        view: the row still responds to touch, which is what the template
        asked for.
      -->
      <an-view
        [style.height]="'26'"
        [accessibilityLabel]="'Volume'"
        [accessibilityValue]="'35 %'"
        [accessibilityState]="off">
        <an-text [fontSize]="10" [color]="'#9fb0d4'">Volume</an-text>
      </an-view>

      <!-- A role taken away on purpose, on top of something that is pressed. -->
      <an-view
        [style.height]="'26'"
        [accessibilityRole]="'none'"
        [accessibilityLabel]="'Pressable decoration'"
        (press)="touch()">
        <an-text [fontSize]="10" [color]="'#64748b'">No role</an-text>
      </an-view>

      <!--
        'expanded' on Android is not a field: it is the action of expanding.
        That is why this row responds to touch; without a '(press)' the host
        refuses to set it and says so in the log, rather than announcing
        something that cannot be done.
      -->
      <an-view
        [style.height]="'26'"
        [accessibilityLabel]="'More options'"
        [accessibilityState]="collapsed"
        (press)="touch()">
        <an-text [fontSize]="10" [color]="'#9fb0d4'">More options</an-text>
      </an-view>

      <!-- 'busy' has no field at all on Android: it goes in words. -->
      <an-view
        [style.height]="'26'"
        [accessibilityLabel]="'Syncing'"
        [accessibilityState]="busy">
        <an-text [fontSize]="10" [color]="'#9fb0d4'">Syncing</an-text>
      </an-view>

      <!--
        A whole row read in one go: one stop, not two. Without this the reader
        stops at the name and again at the subject.
      -->
      <an-view
        [style.height]="'34'"
        [accessible]="true"
        [accessibilityLabel]="'Marta Ruiz, three unread messages'">
        <an-text [fontSize]="10" [color]="'#f4f7ff'">Marta Ruiz</an-text>
        <an-text [fontSize]="9" [color]="'#9fb0d4'">Three unread messages</an-text>
      </an-view>

      <!--
        Decoration is hidden whole, with whatever it carries inside. The text
        below cannot show up in the dump.
      -->
      <an-view [style.height]="'22'" [accessible]="false">
        <an-text [fontSize]="9" [color]="'#334155'">DECORATION NOBODY READS</an-text>
      </an-view>

      <!-- The test identifier, when there is no name covering it. -->
      <an-view [style.height]="'22'" [testID]="'test-row'">
        <an-text [fontSize]="9" [color]="'#64748b'">testID only</an-text>
      </an-view>

      <!-- And when there is: the name wins, because it is the one read out. -->
      <an-view
        [style.height]="'22'"
        [testID]="'covered-row'"
        [accessibilityLabel]="'The name beats the testID'">
        <an-text [fontSize]="9" [color]="'#64748b'">testID and name</an-text>
      </an-view>

      <!--
        The rest of the vocabulary, one row each and nothing inside them.
        There is nothing to look at here: what matters is that every role of
        the contract is asked for at least once, because the dump only checks
        what the template asks for, and a role this screen stopped naming
        would be a role that quietly stopped being checked.
      -->
      <an-view [style.height]="'16'" [accessibilityRole]="'text'" [accessibilityLabel]="'A text'" />
      <an-view [style.height]="'16'" [accessibilityRole]="'image'" [accessibilityLabel]="'A picture'" />
      <an-view [style.height]="'16'" [accessibilityRole]="'radio'" [accessibilityLabel]="'An option'" />
      <an-view [style.height]="'16'" [accessibilityRole]="'switch'" [accessibilityLabel]="'A toggle'" />
      <an-view [style.height]="'16'" [accessibilityRole]="'slider'" [accessibilityLabel]="'A range'" />
      <an-view [style.height]="'16'" [accessibilityRole]="'search'" [accessibilityLabel]="'A search box'" />
      <an-view [style.height]="'16'" [accessibilityRole]="'summary'" [accessibilityLabel]="'A summary'" />

      <!--
        The two system controls, without a single accessibility prop. Material
        already knows what they are and what state they are in, and the host
        has nothing to write over them: if the dump says otherwise, we have
        trampled on it.
      -->
      <an-switch [on]="alerts()" (onChange)="alerts.set($event)" />

      <an-button [title]="'OK'" />
    </an-view>
  `
})
export class AppComponent {
  readonly saved = signal(0)
  readonly alerts = signal(true)

  touch(): void {
    this.saved.set(this.saved())
  }

  // The states live in fields and are not written inline in the template
  // because an object literal in a template is a new object on every change
  // detection pass: the input would change identity without changing content,
  // and the six props would travel to the host again on every touch.
  readonly mixed: NativeAccessibilityState = { checked: 'mixed' }
  readonly checked: NativeAccessibilityState = { checked: true }
  readonly off: NativeAccessibilityState = { selected: true, disabled: true }
  readonly collapsed: NativeAccessibilityState = { expanded: false }
  readonly busy: NativeAccessibilityState = { busy: true }
}
