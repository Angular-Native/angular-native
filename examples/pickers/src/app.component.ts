import { ChangeDetectionStrategy, Component, signal } from '@angular/core'
import { NATIVE_PRIMITIVES, SafeArea } from '@angular-native/primitives'
import type {
  NativeIndexEvent,
  NativeTextEvent,
  NativeValueEvent
} from '@angular-native/primitives'

/**
 * The controls for choosing: segments, dropdown, stepper, search and date.
 *
 * All of them belong to the system wherever the system has them. The two
 * Android does not ship in the platform —the segmented control and the
 * stepper— are drawn with system views respecting their current look, and it
 * is said which is which instead of pretending they are native.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, SafeArea],
  template: `
    <an-view [style.width]="'100%'" [style.height]="'100%'" [backgroundColor]="'#0b1020'">
      <an-safe-area [style.flex]="1" [padding]="20" [style.gap]="18">
        <an-text [fontSize]="26" [fontWeight]="700" [color]="'#f8fafc'">choose</an-text>

        <an-search-bar
          [style.height]="52"
          [placeholder]="'Search…'"
          (input)="onSearch($event)" />
        <an-text [color]="'#94a3b8'" [fontSize]="14">{{ search() || 'nothing searched for' }}</an-text>

        <an-segmented-control
          [style.height]="36"
          [items]="views"
          [selectedIndex]="view()"
          [color]="'#6ee7b7'"
          (change)="onView($event)" />
        <an-text [color]="'#94a3b8'" [fontSize]="14">view: {{ views[view()] }}</an-text>

        <an-view [style.flexDirection]="'row'" [style.alignItems]="'center'" [style.gap]="'16'">
          <an-text [color]="'#cbd5f5'" [style.flexGrow]="'1'">Priority</an-text>
          <an-select
            [style.width]="150"
            [style.height]="40"
            [items]="priorities"
            [selectedIndex]="priority()"
            (change)="priority.set($event.index)" />
        </an-view>

        <an-view [style.flexDirection]="'row'" [style.alignItems]="'center'" [style.gap]="'16'">
          <an-text [color]="'#cbd5f5'" [style.flexGrow]="'1'">Quantity: {{ quantity() }}</an-text>
          <an-stepper
            [style.width]="140"
            [style.height]="40"
            [value]="quantity()"
            [minimumValue]="0"
            [maximumValue]="10"
            (change)="quantity.set($event.value)" />
        </an-view>

        <an-view [style.flexDirection]="'row'" [style.alignItems]="'center'" [style.gap]="'16'">
          <an-text [color]="'#cbd5f5'" [style.flexGrow]="'1'">Date</an-text>
          <an-date-picker
            [style.width]="180"
            [style.height]="40"
            [value]="date()"
            (change)="date.set($event.value)" />
        </an-view>

        <!-- The three button variants, which is what tells them from a text. -->
        <an-view [style.flexDirection]="'row'" [style.gap]="'12'" [style.height]="48">
          <an-button
            [style.flexGrow]="'1'"
            [title]="'Text'"
            [color]="'#6ee7b7'"
            (press)="pressed.set('text')"></an-button>
          <an-button
            [style.flexGrow]="'1'"
            [title]="'Tonal'"
            [variant]="'tonal'"
            [color]="'#6ee7b7'"
            (press)="pressed.set('tonal')"></an-button>
          <an-button
            [style.flexGrow]="'1'"
            [title]="'Filled'"
            [variant]="'filled'"
            [color]="'#6ee7b7'"
            (press)="pressed.set('filled')"></an-button>
        </an-view>
        <an-text [color]="'#94a3b8'" [fontSize]="14">last button: {{ pressed() }}</an-text>
      </an-safe-area>
    </an-view>
  `
})
export class AppComponent {
  readonly views = ['Day', 'Week', 'Month']
  readonly priorities = ['Low', 'Normal', 'High']

  readonly search = signal('')
  readonly view = signal(1)
  readonly priority = signal(1)
  readonly quantity = signal(3)
  readonly date = signal(Date.now())
  readonly pressed = signal('none')

  onSearch(event: NativeTextEvent): void {
    this.search.set(event.value)
  }

  onView(event: NativeIndexEvent): void {
    this.view.set(event.index)
  }

  onQuantity(event: NativeValueEvent): void {
    this.quantity.set(event.value)
  }
}
