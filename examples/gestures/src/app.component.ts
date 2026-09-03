import { ChangeDetectionStrategy, Component, signal } from '@angular/core'
import { NATIVE_PRIMITIVES, SafeArea } from '@angular-native/primitives'
import type {
  NativePanEvent,
  NativePinchEvent,
  NativeRotateEvent
} from '@angular-native/primitives'

/**
 * Gestures and transforms.
 *
 * The card moves with one finger, and scales and rotates with two. None of the
 * three touches the layout: `translateX`, `scale` and `rotate` are applied to
 * the view once it is already placed, so following the finger recomputes
 * nothing.
 *
 * The recognisers are the system's —`UIPanGestureRecognizer` and friends on
 * iOS, `GestureDetector` on Android—, so the thresholds for when a drag starts
 * to count are each platform's own.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, SafeArea],
  template: `
    <an-view
      [style.width]="'100%'"
      [style.height]="'100%'"
      [backgroundColor]="'#0b1020'">
      <an-safe-area
        [style.flex]="1"
        [padding]="20"
        [style.gap]="14">
        <an-text [fontSize]="26" [fontWeight]="700" [color]="'#f8fafc'">
          gestures
        </an-text>

        <an-text [color]="'#94a3b8'" [fontSize]="15">
          {{ status() }}
        </an-text>

        <an-view
          [style.height]="320"
          [backgroundColor]="'#111a33'"
          [borderRadius]="16"
          [style.alignItems]="'center'"
          [style.justifyContent]="'center'">
          <an-view
            [style.width]="140"
            [style.height]="140"
            [backgroundColor]="'#6366f1'"
            [borderRadius]="20"
            [style.alignItems]="'center'"
            [style.justifyContent]="'center'"
            [translateX]="x()"
            [translateY]="y()"
            [scale]="scale()"
            [rotate]="angle()"
            (pan)="onPan($event)"
            (pinch)="onPinch($event)"
            (rotation)="onRotate($event)"
            (doublePress)="reset()"
            (longPress)="onLongPress()">
            <an-text [color]="'#ffffff'" [fontWeight]="600">
              {{ label() }}
            </an-text>
          </an-view>
        </an-view>

        <!--
          The panel grows and fades with an animation. None of this goes back
          through JavaScript: the view is told how long it has to get there, and
          from then on the platform interpolates the changes on its own drawing
          thread.
        -->
        <an-view
          [animate]="260"
          [style.height]="open() ? 160 : 72"
          [style.opacity]="open() ? 1 : 0.55"
          [backgroundColor]="'#1e293b'"
          [borderRadius]="12"
          (press)="open.set(!open())"
          [style.alignItems]="'center'"
          [style.justifyContent]="'center'"
          (swipeLeft)="onSwipe('left')"
          (swipeRight)="onSwipe('right')"
          (swipeUp)="onSwipe('up')"
          (swipeDown)="onSwipe('down')">
          <!--
            This one uses \`[style.fontSize]\` on purpose. The normal and
            recommended way is the typed input \`[fontSize]\`, which the
            compiler checks; this is here so that the other path —the one
            Angular always lets you write— does not quietly come to nothing.
          -->
          <an-text [color]="'#cbd5f5'" [style.fontSize]="18">swipe here: {{ swipe() }}</an-text>
          <an-text [color]="'#64748b'">{{ open() ? 'tap to close' : 'tap to open' }}</an-text>
        </an-view>
      </an-safe-area>
    </an-view>
  `
})
export class AppComponent {
  // What earlier gestures added up to, and what the current gesture has moved
  // so far. They are kept apart because every gesture reports its translation
  // from where it started: adding it straight in would count the same thing
  // twice on every event.
  private committedX = 0
  private committedY = 0
  private committedScale = 1
  private committedAngle = 0

  readonly x = signal(0)
  readonly y = signal(0)
  readonly scale = signal(1)
  readonly angle = signal(0)

  readonly label = signal('drag me')
  readonly status = signal('one finger moves; two scale and rotate')
  readonly swipe = signal('—')
  readonly open = signal(false)


  onPan(event: NativePanEvent): void {
    this.x.set(this.committedX + event.translationX)
    this.y.set(this.committedY + event.translationY)
    if (event.state === 'end') {
      this.committedX = this.x()
      this.committedY = this.y()
      this.status.set(`let go at ${Math.round(event.velocityX)} pt/s`)
    }
    if (event.state === 'cancel') {
      // The system took the gesture away: it goes back to where it was, it is
      // not left halfway wherever the finger stopped counting.
      this.x.set(this.committedX)
      this.y.set(this.committedY)
    }
  }

  onPinch(event: NativePinchEvent): void {
    this.scale.set(this.committedScale * event.scale)
    if (event.state === 'end') {
      this.committedScale = this.scale()
    }
  }

  onRotate(event: NativeRotateEvent): void {
    this.angle.set(this.committedAngle + event.rotation)
    if (event.state === 'end') {
      this.committedAngle = this.angle()
    }
  }

  onLongPress(): void {
    this.label.set('held')
    this.status.set('long press')
  }

  onSwipe(direction: string): void {
    this.swipe.set(direction)
  }

  /** A double tap puts the card back where it belongs. */
  reset(): void {
    this.committedX = 0
    this.committedY = 0
    this.committedScale = 1
    this.committedAngle = 0
    this.x.set(0)
    this.y.set(0)
    this.scale.set(1)
    this.angle.set(0)
    this.label.set('drag me')
    this.status.set('reset')
  }
}
