import { ChangeDetectionStrategy, Component, computed, input, signal } from '@angular/core'

import { type NativeSafeAreaInsets } from './primitives'

/**
 * Keeps the content clear of whatever the system is covering it with: the
 * notch, the status bar, the home indicator, Android's navigation bar, the
 * round bezel of a watch — and the keyboard.
 *
 * The margins are neither constant nor computable: they change on rotation, on
 * going into split screen, and when the keyboard comes up. The host measures
 * them every time they change and reports the lot through one `safeArea`
 * event, so a template asks one question and gets one answer.
 *
 * **The keyboard is in the bottom inset.** It is not a sum: while the keyboard
 * is up it is drawn over the home indicator and the navigation bar, so what
 * arrives is the larger of the two. A form whose last field would otherwise sit
 * under the keyboard therefore needs nothing but `'bottom'` among its edges —
 * and, symmetrically, a safe area that does *not* list `'bottom'` is asking not
 * to be kept clear of the bottom of the screen, keyboard included.
 *
 * The inset does not jump to its final value: it arrives once per frame,
 * following the animation the system is already running, so the layout travels
 * with the keyboard instead of arriving before it. On iOS that is the duration
 * and the curve out of the keyboard notification; on Android it is
 * `WindowInsetsAnimation.Callback`, which exists from API 30 — below that the
 * keyboard is not reported at all and a field at the bottom stays under it.
 * See `AnHost.installWindowInsets` for why that gap is not papered over.
 *
 * ```html
 * <an-safe-area [edges]="['top', 'bottom']" [padding]="20" [style.gap]="'12'">
 *   <an-text>no longer sitting under the notch</an-text>
 * </an-safe-area>
 * ```
 *
 * There is no view inside: the safe area *is* its view, so whatever is put on it
 * to arrange its children —`gap`, `flexDirection`, `alignItems`— governs them.
 * With an intermediate view it did not: the styles stayed on the wrapper, which
 * had only one child, and nothing happened. No noise, no error, no spacing.
 *
 * The inset is applied as padding and not as margin on purpose: the margins the
 * system reserves are measured for the view they belong to, so a view that moved
 * itself out of the way with a margin would stop being under the notch, would
 * start reserving zero, would go back to where it was, and on and on for ever.
 */
@Component({
  selector: 'an-safe-area',
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    '[style.flexGrow]': "'1'",
    '[style.minHeight]': "'0'",
    '[style.paddingTop]': 'space().top',
    '[style.paddingRight]': 'space().right',
    '[style.paddingBottom]': 'space().bottom',
    '[style.paddingLeft]': 'space().left',
    '(safeArea)': 'onInsets($event)'
  },
  template: `<ng-content />`
})
export class SafeArea {
  /** Which edges to keep clear. All four by default. */
  readonly edges = input<readonly ('top' | 'right' | 'bottom' | 'left')[]>([
    'top',
    'right',
    'bottom',
    'left'
  ])

  /**
   * Padding of your own, added on top of what the system reserves.
   *
   * It is an input and not a `[style.padding]` because this view's padding is
   * already written by the safe area: the two of them at once would overwrite
   * each other, and whichever lost would do so in silence.
   */
  readonly padding = input(0)

  /**
   * In a host listener, `$event` is typed as an `Event` and there is no way to
   * tell Angular that this one carries something else. The cast lives here, in
   * one place, rather than in every template.
   */
  protected onInsets(event: Event): void {
    this.insets.set(event as unknown as NativeSafeAreaInsets)
  }

  protected readonly insets = signal<NativeSafeAreaInsets>({
    top: 0,
    right: 0,
    bottom: 0,
    left: 0
  })

  protected readonly space = computed(() => {
    const insets = this.insets()
    const edges = this.edges()
    const extra = this.padding()
    const only = (edge: 'top' | 'right' | 'bottom' | 'left') =>
      String((edges.includes(edge) ? insets[edge] : 0) + extra)
    return {
      top: only('top'),
      right: only('right'),
      bottom: only('bottom'),
      left: only('left')
    }
  })
}
