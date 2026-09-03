import { ChangeDetectionStrategy, Component, signal } from '@angular/core'
import { NATIVE_PRIMITIVES, SafeArea } from '@angular-native/primitives'
import type { NativeIndexEvent, NativeValueEvent } from '@angular-native/primitives'

/**
 * Every control the core asks the host to measure, with nobody giving it a size.
 *
 * It is not meant to be looked at either: it is meant to be measured. Not one
 * row here writes `[style.width]` or `[style.height]`, which is the whole point
 * — with them, `measure_control` is never consulted and a host that answers
 * zero looks exactly like a host that answers properly.
 *
 * That is how seven of them stayed invisible on Android: `examples/pickers`
 * sizes each control by hand, so it went on passing while `an-stepper`,
 * `an-date-picker` and five more were laid out 0x0 on a real phone.
 *
 * Each one carries an `[accessibilityLabel]`, because what reads this screen is
 * `uiautomator dump` in `scripts/check-measure-device.sh`, and that is the name
 * a node comes out under.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, SafeArea],
  template: `
    <an-view
      [style.width]="'100%'"
      [style.height]="'100%'"
      [style.gap]="'8'"
      [backgroundColor]="'#0b1020'">
      <an-safe-area [edges]="['top']">
      <an-navigation-bar [title]="'measure'" [accessibilityLabel]="'the navigation bar'" />

      <an-view [style.padding]="'16'" [style.gap]="'12'">
        <an-search-bar
          [placeholder]="'Search…'"
          [accessibilityLabel]="'the search bar'"
          (input)="typed.set($event.value)" />

        <an-segmented-control
          [items]="views"
          [selectedIndex]="view()"
          [color]="'#6ee7b7'"
          [accessibilityLabel]="'the segmented control'"
          (change)="onView($event)" />

        <an-select
          [items]="priorities"
          [selectedIndex]="priority()"
          [accessibilityLabel]="'the dropdown'"
          (change)="priority.set($event.index)" />

        <an-stepper
          [value]="quantity()"
          [minimumValue]="0"
          [maximumValue]="10"
          [accessibilityLabel]="'the stepper'"
          (change)="onQuantity($event)" />

        <an-date-picker
          [value]="date()"
          [accessibilityLabel]="'the date picker'"
          (change)="date.set($event.value)" />

        <an-view [style.flexDirection]="'row'" [style.gap]="'12'" [style.alignItems]="'center'">
          <an-icon [name]="'home'" [color]="'#6ee7b7'" [accessibilityLabel]="'the icon'" />
          <an-text [color]="'#94a3b8'" [fontSize]="14">{{ typed() || 'nothing typed' }}</an-text>
        </an-view>
      </an-view>
      </an-safe-area>
    </an-view>
  `
})
export class AppComponent {
  readonly views = ['List', 'Grid']
  readonly priorities = ['Low', 'Normal', 'High']

  readonly view = signal(0)
  readonly priority = signal(1)
  readonly quantity = signal(3)
  readonly date = signal(Date.now())
  readonly typed = signal('')

  onView(event: NativeIndexEvent): void {
    this.view.set(event.index)
  }

  onQuantity(event: NativeValueEvent): void {
    this.quantity.set(event.value)
  }
}
