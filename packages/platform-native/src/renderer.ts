import { Renderer2, RendererFactory2, RendererStyleFlags2, type ListenerOptions } from '@angular/core'

import {
  attach,
  createCommentNode,
  createElementNode,
  createTextNode,
  detach,
  dom,
  markDestroyed,
  materialize,
  NativeNode
} from './native-node'
import { KNOWN_STYLES } from './style-names'

/**
 * Text props that in CSS would be styles and here are not.
 *
 * A font's size and weight are not box styling: layout needs them to measure and
 * the host needs them to draw, and both read them off the props. Without this,
 * writing `[style.fontSize]` in a template —which Angular accepts without a
 * murmur— did nothing at all: the text was measured in the default font and
 * drawn in UIKit's, which is not the same one, and the text spilled out of its
 * box and the parent clipped it. It looked like text disappearing.
 *
 * The recommended way is still the typed input, `[fontSize]`, which the compiler
 * checks. This is so the other one does not lie.
 */
const TEXT_PROPS = new Map<string, string>(
  [
    'fontSize',
    'fontWeight',
    'fontStyle',
    'fontFamily',
    'lineHeight',
    'letterSpacing',
    'color',
    'textAlign',
    'numberOfLines'
    // Angular normalises style names to hyphens before they get here, so both
    // spellings have to be recognised: the one written in the template and the
    // one that arrives.
  ].flatMap((name) => {
    const dashed = name.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)
    return [
      [name, name],
      [dashed, name]
    ] as [string, string][]
  })
)

/**
 * Warns once per style name that nobody is going to look at.
 *
 * Angular lets you write `[style.whatever]` without a murmur, and up to here
 * that travelled to the host like any other prop: the host did not use it and
 * nothing happened. No error and no trace, which is what makes these bugs
 * expensive —four of them turned up in a single day, and all four looked like
 * "this does nothing".
 *
 * The warning fires once per name: the same style gets written on every change
 * detection pass, and warning every time would fill the log without saying
 * anything new.
 */
const warned = new Set<string>()

function warnUnknownStyle(name: string): void {
  // The core accepts both spellings, so this does too: otherwise every style
  // written in camel case —which Angular hands over hyphenated— would raise a
  // false warning.
  const camel = name.replace(/-([a-z])/g, (_, letter: string) => letter.toUpperCase())
  if (KNOWN_STYLES.has(camel) || TEXT_PROPS.has(camel) || warned.has(camel)) return
  warned.add(camel)
  console.warn(
    `[angular-native] nadie reconoce el estilo "${name}", así que no hará nada. ` +
      'Si es una propiedad del control —color, título, valor— va como entrada ' +
      'tipada, no como estilo.'
  )
}

/**
 * `Renderer2` over native primitives.
 *
 * This class is the entire seam between Angular and the core: templates compiled
 * by AOT emit instructions that end up here, so an Angular template works
 * untouched — the only thing that changes is that the elements are `<an-view>`
 * and `<an-text>` instead of `<div>` and `<span>`.
 */
export class NativeRenderer extends Renderer2 {
  /** Classes accumulated per node, so they can be sent to the core together. */
  private readonly classes = new WeakMap<NativeNode, Set<string>>()

  constructor(private readonly root: NativeNode) {
    super()
  }

  readonly data: { [key: string]: unknown } = Object.create(null)

  destroy(): void {}

  createElement(name: string): NativeNode {
    return createElementNode(name)
  }

  createComment(): NativeNode {
    return createCommentNode()
  }

  createText(value: string): NativeNode {
    return createTextNode(value)
  }

  override destroyNode = (node: NativeNode): void => {
    if (node.destroyed) return
    // It is asked before marking: `mounted` means «materialised and alive», so
    // marking first sets it to `false` and the removal would never reach the
    // core. Normally nobody notices —Angular takes things out of the tree before
    // destroying them— but on a hot refresh it destroys first, and then the old
    // screen was left sitting underneath the new one.
    const wasMounted = node.mounted
    markDestroyed(node)
    if (wasMounted) dom.destroyNode(node.id)
  }

  appendChild(parent: NativeNode, child: NativeNode): void {
    attach(parent, child, parent.children.length)
  }

  insertBefore(parent: NativeNode, child: NativeNode, ref: NativeNode | null): void {
    if (!parent) return
    const at = ref ? parent.indexOf(ref) : -1
    attach(parent, child, at === -1 ? parent.children.length : at)
  }

  removeChild(parent: NativeNode | null, child: NativeNode): void {
    const from = parent ?? child.parent
    if (from) detach(from, child)
  }

  /**
   * Angular passes the root component's selector here. There is no
   * `querySelector` worth having: the platform pins the root down at startup.
   */
  selectRootElement(): NativeNode {
    return this.root
  }

  parentNode(node: NativeNode): NativeNode | null {
    return node.parent
  }

  nextSibling(node: NativeNode): NativeNode | null {
    const parent = node.parent
    if (!parent) return null
    return parent.children[parent.indexOf(node) + 1] ?? null
  }

  setAttribute(el: NativeNode, name: string, value: string): void {
    if (!this.writable(el)) return
    dom.setProp(el.id, name, value)
  }

  removeAttribute(el: NativeNode, name: string): void {
    if (!this.writable(el)) return
    dom.setProp(el.id, name, null)
  }

  /**
   * Gets the node ready to receive something.
   *
   * A component's wrapper does not exist in the core until somebody puts a
   * style, a prop or a listener on it: at that moment it stops being a wrapper.
   */
  private writable(node: NativeNode): boolean {
    if (node.destroyed || node.kind === 'Comment') return false
    if (!node.materialized) materialize(node)
    return node.materialized
  }

  /**
   * There are no stylesheets and no cascade here, so a class cannot resolve
   * itself. They are accumulated and sent as a `className` prop: what gets done
   * with them is the styling layer's business, not the renderer's.
   */
  addClass(el: NativeNode, name: string): void {
    let set = this.classes.get(el)
    if (!set) this.classes.set(el, (set = new Set()))
    if (set.has(name)) return
    set.add(name)
    this.flushClasses(el, set)
  }

  removeClass(el: NativeNode, name: string): void {
    const set = this.classes.get(el)
    if (!set?.delete(name)) return
    this.flushClasses(el, set)
  }

  private flushClasses(el: NativeNode, set: Set<string>): void {
    if (!this.writable(el)) return
    dom.setProp(el.id, 'className', [...set].join(' '))
  }

  setStyle(el: NativeNode, style: string, value: unknown, flags?: RendererStyleFlags2): void {
    if (!this.writable(el)) return
    warnUnknownStyle(style)
    // `!important` means nothing without a cascade: the flag is ignored and the
    // value applied, which is the only behaviour possible here.
    void flags
    const prop = TEXT_PROPS.get(style)
    if (prop) {
      dom.setProp(el.id, prop, value as never)
      return
    }
    dom.setStyle(el.id, style, value === null || value === undefined ? '' : String(value))
  }

  removeStyle(el: NativeNode, style: string): void {
    if (!this.writable(el)) return
    const prop = TEXT_PROPS.get(style)
    if (prop) {
      dom.setProp(el.id, prop, null as never)
      return
    }
    dom.setStyle(el.id, style, '')
  }

  setProperty(el: NativeNode, name: string, value: unknown): void {
    if (!this.writable(el)) return
    dom.setProp(el.id, name, value as never)
  }

  /** `Renderer2.setValue` on a text node. */
  setValue(node: NativeNode, value: string): void {
    if (node.kind === 'RawText' && !node.destroyed) dom.setText(node.id, value)
  }

  listen(
    target: 'window' | 'document' | 'body' | NativeNode,
    event: string,
    callback: (payload: unknown) => boolean | void,
    options?: ListenerOptions
  ): () => void {
    void options
    if (typeof target === 'string') {
      // There is no window and no document to listen to. App events
      // (background, keyboard) will come through a native module, not here.
      return () => {}
    }
    if (!this.writable(target)) return () => {}
    const unlisten = dom.listen(target.id, event, (payload) => {
      callback(payload)
    })
    // Angular drops the subscriptions when it destroys the view, and by then
    // the node no longer exists in the core.
    return () => {
      if (!target.destroyed) unlisten()
    }
  }
}

/**
 * One renderer for the whole app: with no style encapsulation there is nothing
 * to isolate per component, so creating one per view would only burn memory.
 */
export class NativeRendererFactory extends RendererFactory2 {
  private readonly renderer: NativeRenderer

  constructor(root: NativeNode) {
    super()
    this.renderer = new NativeRenderer(root)
  }

  createRenderer(): Renderer2 {
    return this.renderer
  }

  // Hooks belonging to the browser's animation renderer. There is nothing here
  // to batch and nothing to wait for: the core already batches per frame.
  begin(): void {}
  end(): void {}
  whenRenderingDone(): Promise<unknown> {
    return Promise.resolve(null)
  }
}
