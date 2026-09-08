import { ChangeDetectionStrategy, Component, computed, signal } from '@angular/core'
import { NATIVE_PRIMITIVES, type NativeCrownEvent } from '@angular-native/primitives'

/**
 * What the watch knows how to paint, across three screens.
 *
 * They go inside an `<an-stack-view>` rather than one below the other because
 * everything does not fit in 208 points of width, and because that way the stack
 * itself can be seen too: only the top screen is painted, and it slides in and
 * out.
 *
 * Every control is a system one. An `<an-switch>` is a `Toggle`, an
 * `<an-select>` is the wheel that turns with the crown, and an
 * `<an-date-picker>` opens watchOS's own dial picker: there is not one drawing
 * that merely looks like them.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-stack-view
      [style.width]="'100%'"
      [style.flexGrow]="'1'"
      [backgroundColor]="'#0b1020'"
      [transition]="direction()">

      @if (page() === 0) {
        <an-view [style.width]="'100%'" [style.height]="'100%'">
          <an-scroll-view [style.width]="'100%'" [style.flexGrow]="'1'">
            <an-view
              [style.paddingTop]="'40'"
              [style.paddingHorizontal]="'10'"
              [style.paddingBottom]="'16'"
              [style.gap]="'10'"
              [style.width]="'100%'">

              <!--
                The one file in this example that is not code.
                resources/logo.png sits beside src/, and an copies it into the
                .app under that same name: no scheme, no URL, no network. Until
                the watch build learned to carry an app's resources this example
                had no image at all, because an example in this repository
                cannot be made to depend on a server being up.
              -->
              <an-view [style.flexDirection]="'row'" [style.alignItems]="'center'" [style.gap]="'8'" [style.width]="'100%'">
                <an-image
                  [source]="'logo.png'"
                  [resizeMode]="'contain'"
                  [style.width]="'22'"
                  [style.height]="'22'"></an-image>
                <an-text [fontSize]="17" [fontWeight]="'bold'" [color]="'#f4f7ff'">controls</an-text>
              </an-view>

              <!--
                The label and the control are two nodes, not one: what shares out
                the width is taffy, and a control with a label inside it would be
                SwiftUI deciding the layout on its own.
              -->
              <an-view [style.flexDirection]="'row'" [style.alignItems]="'center'" [style.width]="'100%'">
                <an-text [style.flexGrow]="'1'" [fontSize]="14" [color]="'#9fb0d4'">alerts</an-text>
                <an-switch [style.width]="'60'" [(on)]="alerts" [color]="'#6ee7b7'"></an-switch>
              </an-view>

              <an-text [fontSize]="14" [color]="'#9fb0d4'">brightness {{ brightness().toFixed(0) }}%</an-text>
              <an-slider
                [style.width]="'100%'"
                [(value)]="brightness"
                [minimumValue]="0"
                [maximumValue]="100"
                [color]="'#6ee7b7'"></an-slider>
              <an-progress-bar [style.width]="'100%'" [progress]="brightness() / 100" [color]="'#6ee7b7'"></an-progress-bar>

              <an-text [fontSize]="14" [color]="'#9fb0d4'">sets {{ sets() }}</an-text>
              <an-stepper
                [style.width]="'100%'"
                [value]="sets()"
                [minimumValue]="0"
                [maximumValue]="12"
                [step]="1"
                (change)="sets.set($event.value)"></an-stepper>

              <an-view [style.flexDirection]="'row'" [style.alignItems]="'center'" [style.gap]="'10'" [style.width]="'100%'">
                <an-activity-indicator [color]="'#f59e0b'"></an-activity-indicator>
                <an-icon [name]="'favorite'" [size]="22" [color]="'#f472b6'"></an-icon>
                <an-icon [name]="'bell'" [size]="22" [color]="'#60a5fa'"></an-icon>
                <an-text [style.flexGrow]="'1'" [fontSize]="12" [color]="'#64748b'">SF Symbols</an-text>
              </an-view>

              <!--
                Transforms take no part in the layout: the badge is
                permanently tilted and still occupies its upright box, so the
                row does not grow to make room for the corners. That is what
                makes them cheap, and it is why they are what you follow a
                finger with.
              -->
              <an-view [style.flexDirection]="'row'" [style.alignItems]="'center'" [style.gap]="'12'" [style.width]="'100%'" [style.height]="'26'">
                <an-view
                  [style.width]="'40'"
                  [style.height]="'18'"
                  [style.alignItems]="'center'"
                  [style.justifyContent]="'center'"
                  [backgroundColor]="'#f472b6'"
                  [borderRadius]="4"
                  [rotate]="-0.26">
                  <an-text [fontSize]="10" [color]="'#0b1020'">beta</an-text>
                </an-view>

                <!--
                  [animate] is set once and holds for every change after it, so
                  the button below moves and grows this dot over 220 ms instead
                  of jumping. What it covers is the frame, the opacity and the
                  transform, and nothing else: a colour change is still a cut.
                -->
                <an-view
                  [style.width]="'16'"
                  [style.height]="'16'"
                  [backgroundColor]="'#6ee7b7'"
                  [borderRadius]="8"
                  [animate]="220"
                  [animateEasing]="'ease-out'"
                  [translateX]="moved() ? 56 : 0"
                  [scale]="moved() ? 1.6 : 1"></an-view>
              </an-view>

              <an-button
                [title]="moved() ? '← bring the dot back' : 'move the dot →'"
                [color]="'#0b1020'"
                [backgroundColor]="'#f472b6'"
                [borderRadius]="10"
                (press)="moved.set(!moved())"></an-button>

              <an-button
                [title]="'input →'"
                [color]="'#0b1020'"
                [backgroundColor]="'#6ee7b7'"
                [borderRadius]="10"
                (press)="go(1)"></an-button>
            </an-view>
          </an-scroll-view>
        </an-view>
      }

      @if (page() === 1) {
        <an-view [style.width]="'100%'" [style.height]="'100%'">
          <an-scroll-view [style.width]="'100%'" [style.flexGrow]="'1'">
            <an-view
              [style.paddingTop]="'40'"
              [style.paddingHorizontal]="'10'"
              [style.paddingBottom]="'16'"
              [style.gap]="'10'"
              [style.width]="'100%'">

              <an-text [fontSize]="17" [fontWeight]="'bold'" [color]="'#f4f7ff'">input</an-text>

              <!--
                On the watch a text field is not typed into where it sits: on
                being tapped the system opens its own screen —dictation,
                scribble or keyboard— and hands the result back.
              -->
              <!--
                The height is set by hand and not by the text: on the watch the
                field brings a container of its own, taller than one line, and
                without this it would overlap what is below. The host says so in
                the log if it is forgotten.
              -->
              <an-text-input
                [style.width]="'100%'"
                [style.height]="'44'"
                [placeholder]="'name'"
                [(value)]="name"
                [color]="'#f4f7ff'"
                [fontSize]="15"></an-text-input>
              <an-text [fontSize]="12" [color]="'#64748b'">hello, {{ name() || 'nobody' }}</an-text>

              <an-text [fontSize]="14" [color]="'#9fb0d4'">pace: {{ paces[pace()] }}</an-text>
              <an-select
                [style.width]="'100%'"
                [items]="paces"
                [selectedIndex]="pace()"
                (change)="pace.set($event.index)"></an-select>

              <an-text [fontSize]="14" [color]="'#9fb0d4'">at</an-text>
              <an-date-picker
                [style.width]="'100%'"
                [mode]="'time'"
                [value]="time()"
                (change)="time.set($event.value)"></an-date-picker>

              <an-button
                [title]="'crown →'"
                [color]="'#0b1020'"
                [backgroundColor]="'#60a5fa'"
                [borderRadius]="10"
                (press)="go(2)"></an-button>
            </an-view>
          </an-scroll-view>
        </an-view>
      }

      @if (page() === 2) {
        <!--
          This screen carries no an-scroll-view on purpose. On the watch the
          crown belongs to whoever has the focus, and a ScrollView keeps it until
          something else is touched: without one, the box below takes it on its
          own as soon as it appears.
        -->
        <an-view [style.width]="'100%'" [style.height]="'100%'">
            <an-view
              [style.paddingTop]="'40'"
              [style.paddingHorizontal]="'10'"
              [style.gap]="'8'"
              [style.width]="'100%'">

              <an-text [fontSize]="17" [fontWeight]="'bold'" [color]="'#f4f7ff'">crown</an-text>

              <!--
                The (swipeLeft) and the (longPress) go on the same box as the
                crown so it can be seen that they coexist: the crown goes to the
                view that has the focus and the gestures to the finger, and they
                do not get in each other's way.
              -->
              <an-view
                [style.width]="'100%'"
                [style.height]="'64'"
                [style.alignItems]="'center'"
                [style.justifyContent]="'center'"
                [borderRadius]="12"
                [backgroundColor]="'#152036'"
                (crown)="turn($event)"
                (longPress)="reset()"
                (swipeLeft)="go(1)">
                <an-text [fontSize]="26" [fontWeight]="'bold'" [color]="'#f59e0b'">{{ steps() }}</an-text>
                <an-text [fontSize]="11" [color]="'#64748b'">turn the crown</an-text>
              </an-view>
              <an-text [fontSize]="11" [color]="'#64748b'">
                speed {{ speed().toFixed(2) }} · long press to reset
              </an-text>

              <an-button
                [title]="'alert'"
                [color]="'#0b1020'"
                [backgroundColor]="'#f59e0b'"
                [borderRadius]="10"
                (press)="dialog.set(true)"></an-button>

              <an-button
                [title]="'detail'"
                [color]="'#f4f7ff'"
                [backgroundColor]="'#2b1e4a'"
                [borderRadius]="10"
                (press)="sheet.set(true)"></an-button>

              <an-button
                [title]="'← controls'"
                [color]="'#9fb0d4'"
                [backgroundColor]="'#152036'"
                [borderRadius]="10"
                (press)="go(0)"></an-button>
            </an-view>
        </an-view>
      }
    </an-stack-view>

    <!--
      The dialog and the sheet take up no room: the system presents them over
      everything. That is why they can live here, outside the stack, and it does
      not matter which page is being looked at.
    -->
    <an-alert
      [visible]="dialog()"
      [title]="'battery'"
      [message]="brightness().toFixed(0) + ' per cent left'"
      [buttons]="answers"
      (select)="answer($event)"></an-alert>

    <!--
      Absolute and full screen. An an-modal is an ordinary node as far as the
      layout is concerned, so if it is left in the flow it eats its share of the
      column —on a watch, half the screen— even while it is not visible. The
      frame given to it here is also the size taffy lays its contents out with,
      and a watch sheet takes up the whole screen.
    -->
    <!--
      The paddingTop inside is 56 and not 40: the close button of a sheet is
      painted by the system at the top left, and room has to be left for it.
    -->
    <an-modal
      [style.position]="'absolute'"
      [style.top]="'0'"
      [style.left]="'0'"
      [style.width]="'100%'"
      [style.height]="'100%'"
      [visible]="sheet()"
      [presentation]="'sheet'"
      (dismiss)="sheet.set(false)">
      <an-view
        [style.width]="'100%'"
        [style.height]="'100%'"
        [style.paddingTop]="'56'"
        [style.paddingHorizontal]="'12'"
        [style.gap]="'8'"
        [backgroundColor]="'#101827'">
        <an-text [fontSize]="16" [fontWeight]="'bold'" [color]="'#f4f7ff'">detail</an-text>
        <an-text [fontSize]="13" [color]="'#9fb0d4'">
          This is a system sheet, not a layer drawn on top: it is pulled down with a finger.
        </an-text>
        <an-text [fontSize]="12" [color]="'#64748b'">last answer: {{ lastAnswer() }}</an-text>
        <an-button
          [title]="'close'"
          [color]="'#0b1020'"
          [backgroundColor]="'#6ee7b7'"
          [borderRadius]="10"
          (press)="sheet.set(false)"></an-button>
      </an-view>
    </an-modal>
  `
})
export class AppComponent {
  readonly page = signal(0)
  readonly direction = signal<'push' | 'pop'>('push')

  readonly alerts = signal(true)
  /**
   * Where the dot is. It starts moved so that the example shows a transform in
   * effect on the frame it mounts, rather than only after somebody presses
   * something.
   */
  readonly moved = signal(true)
  readonly brightness = signal(40)
  readonly sets = signal(3)

  readonly name = signal('')
  readonly paces = ['easy', 'normal', 'hard']
  readonly pace = signal(1)
  readonly time = signal(Date.now())

  /**
   * How many steps the crown has turned.
   *
   * The running total is kept with decimals and only rounded when shown: if it
   * were rounded on adding, half a detent would be lost on every event and
   * turning slowly would never move the number.
   */
  private turned = 0
  readonly steps = signal(0)
  readonly speed = signal(0)

  readonly dialog = signal(false)
  readonly sheet = signal(false)
  readonly answers = ['ok', 'not now']
  /** Index of the button pressed in the dialog; -1 while none has been. */
  readonly chosen = signal(-1)
  readonly lastAnswer = computed(() =>
    this.chosen() < 0 ? 'none' : this.answers[this.chosen()]
  )

  /**
   * One crown event.
   *
   * It carries `delta` —how far it has turned since the previous event— and not
   * only the running total, which is what is almost always wanted: adding the
   * step to what was already there without having to remember where it was.
   */
  turn(event: NativeCrownEvent): void {
    this.turned = Math.max(0, this.turned + event.delta)
    this.steps.set(Math.round(this.turned))
    this.speed.set(event.velocity)
  }

  reset(): void {
    this.turned = 0
    this.steps.set(0)
  }

  /** The button of an `<an-alert>` arrives as its index in the list. */
  answer(index: number): void {
    this.chosen.set(index)
    this.dialog.set(false)
  }

  go(target: number): void {
    this.direction.set(target > this.page() ? 'push' : 'pop')
    this.page.set(target)
  }
}
