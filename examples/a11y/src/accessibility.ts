import { Directive, ElementRef, Renderer2, effect, inject, input } from '@angular/core'
import type { NativeAccessibilityState, NativeRole } from '@angular-native/primitives'

/**
 * The bridge the contract is missing, living here because this example is not
 * allowed to touch `packages/primitives`.
 *
 * `NativeVisual` declares the six accessibility inputs, but its `push({...})`
 * — the effect that sends the core whatever the template wrote — does not list
 * them yet. A declared input that is never pushed is worse than no input at
 * all: Angular matches it against the directive and keeps it, so the template
 * writes it, the compiler accepts it, and nothing reaches the host. Without
 * this bridge, everything underneath this screen cannot be seen running.
 *
 * It invents nothing. The names it sends are the contract's and the ones the
 * Android host recognises, and the `JSON.stringify` of the state is the same
 * treatment `items` and `buttons` already get, which are objects too. The day
 * `NativeVisual` pushes its six inputs, this file gets deleted and the
 * template does not change a single line.
 *
 * Two directives matching the same tag and declaring the same input is legal
 * in Angular: the assignment reaches both. Here `View` receives it and does
 * nothing with it, and this one sends it.
 */
@Directive({
  selector: 'an-view,an-text,an-button,an-switch,an-image'
})
export class Accessibility {
  private readonly node = inject(ElementRef).nativeElement
  private readonly renderer = inject(Renderer2)

  readonly accessibilityLabel = input<string | null>(null)
  readonly accessibilityHint = input<string | null>(null)
  readonly accessibilityRole = input<NativeRole | null>(null)
  readonly accessibilityValue = input<string | null>(null)
  readonly accessibilityState = input<NativeAccessibilityState | null>(null)
  readonly accessible = input<boolean | null>(null)

  constructor() {
    // A literal copy of `NativeVisual.push`: one single effect for the six,
    // only what changed travels, and on the first pass the nulls keep quiet,
    // which is what an input nobody has set is worth.
    const props: Record<string, () => unknown> = {
      accessibilityLabel: this.accessibilityLabel,
      accessibilityHint: this.accessibilityHint,
      accessibilityRole: this.accessibilityRole,
      accessibilityValue: this.accessibilityValue,
      accessibilityState: () => {
        const state = this.accessibilityState()
        return state === null ? null : JSON.stringify(state)
      },
      accessible: this.accessible
    }
    const entries = Object.entries(props)
    const sent = new Map<string, unknown>()
    let first = true
    effect(() => {
      for (const [name, read] of entries) {
        const value = read()
        if (sent.has(name) && Object.is(sent.get(name), value)) continue
        sent.set(name, value)
        if (first && (value === null || value === undefined)) continue
        this.renderer.setProperty(this.node, name, value ?? null)
      }
      first = false
    })
  }
}
