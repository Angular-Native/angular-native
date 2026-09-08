import { ChangeDetectionStrategy, Component } from '@angular/core'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'

/**
 * What a border is here, and what the four per-side widths are not.
 *
 * The line is `[borderWidth]`, a prop: one number, one outline, drawn by every
 * host inside the frame taffy gave. `borderTopWidth` and its three siblings are
 * *styles*: taffy resolves them, they push the children in and they never reach
 * a host as anything to draw.
 *
 * Both halves are here so the check can see them apart: the prop shows up in
 * the headless dump as a prop and moves no child, the style moves the child and
 * shows up nowhere.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-view [style.flexGrow]="'1'" [backgroundColor]="'#0b0e14'" [style.padding]="'0'">
      <!-- The drawn border. The prop reaches the host; the child stays at 0,0
           inside the parent, because a a CALayer border is painted over the
           content and does not move it. -->
      <an-view
        [testID]="'drawn'"
        [borderWidth]="4"
        [borderColor]="'#6ee7b7'"
        [style.width]="'200'"
        [style.height]="'60'"
      >
        <an-view [testID]="'drawn-child'" [style.flexGrow]="'1'" [backgroundColor]="'#1e293b'" />
      </an-view>

      <!-- The layout ones. The child is pushed in by the four numbers and no
           host is told a thing. -->
      <an-view
        [testID]="'inset'"
        [style.borderTopWidth]="'2'"
        [style.borderRightWidth]="'4'"
        [style.borderBottomWidth]="'8'"
        [style.borderLeftWidth]="'16'"
        [style.width]="'200'"
        [style.height]="'60'"
      >
        <an-view [testID]="'inset-child'" [style.flexGrow]="'1'" [backgroundColor]="'#1e293b'" />
      </an-view>

      <!-- The uniform style, which is the prop's own name written as a style.
           It insets like the four above and draws like none of them. -->
      <an-view
        [testID]="'uniform'"
        [style.borderWidth]="'6'"
        [style.width]="'200'"
        [style.height]="'60'"
      >
        <an-view [testID]="'uniform-child'" [style.flexGrow]="'1'" [backgroundColor]="'#1e293b'" />
      </an-view>
    </an-view>
  `
})
export class AppComponent {}
