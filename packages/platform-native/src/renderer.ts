import { Renderer2, RendererFactory2, RendererStyleFlags2, type ListenerOptions } from '@angular/core'

import {
  attach,
  createCommentNode,
  createElementNode,
  createTextNode,
  detach,
  dom,
  markDestroyed,
  NativeNode
} from './native-node'

/**
 * `Renderer2` sobre primitivas nativas.
 *
 * Esta clase es toda la costura entre Angular y el core: las plantillas
 * compiladas por AOT emiten instrucciones que acaban aquí, así que una
 * plantilla Angular funciona sin tocarla — lo único que cambia es que los
 * elementos son `<View>` y `<Text>` en vez de `<div>` y `<span>`.
 */
export class NativeRenderer extends Renderer2 {
  /** Clases acumuladas por nodo, para poder mandarlas juntas al core. */
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
    markDestroyed(node)
    if (node.mounted) dom.destroyNode(node.id)
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
   * Angular pasa aquí el selector del componente raíz. No hay `querySelector`
   * que valga: la raíz la fija la plataforma al arrancar.
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
    if (el.mounted && !el.destroyed) dom.setProp(el.id, name, value)
  }

  removeAttribute(el: NativeNode, name: string): void {
    if (el.mounted && !el.destroyed) dom.setProp(el.id, name, null)
  }

  /**
   * Aquí no hay hojas de estilo ni cascada, así que una clase no puede
   * resolverse sola. Se acumulan y se mandan como prop `className`: lo que haga
   * con ellas es cosa de la capa de estilos, no del renderer.
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
    if (el.mounted && !el.destroyed) dom.setProp(el.id, 'className', [...set].join(' '))
  }

  setStyle(el: NativeNode, style: string, value: unknown, flags?: RendererStyleFlags2): void {
    if (!el.mounted || el.destroyed) return
    // `!important` no significa nada sin cascada: se ignora la bandera y se
    // aplica el valor, que es el único comportamiento posible aquí.
    void flags
    dom.setStyle(el.id, style, value === null || value === undefined ? '' : String(value))
  }

  removeStyle(el: NativeNode, style: string): void {
    if (el.mounted && !el.destroyed) dom.setStyle(el.id, style, '')
  }

  setProperty(el: NativeNode, name: string, value: unknown): void {
    if (el.mounted && !el.destroyed) dom.setProp(el.id, name, value as never)
  }

  /** `Renderer2.setValue` sobre un nodo de texto. */
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
      // No hay ventana ni documento a los que escuchar. Los eventos de app
      // (background, teclado) llegarán por un módulo nativo, no por aquí.
      return () => {}
    }
    if (!target.mounted || target.destroyed) return () => {}
    const unlisten = dom.listen(target.id, event, (payload) => {
      callback(payload)
    })
    // Angular suelta las suscripciones al destruir la vista, y para entonces
    // el nodo ya no existe en el core.
    return () => {
      if (!target.destroyed) unlisten()
    }
  }
}

/**
 * Un único renderer para toda la app: sin encapsulación de estilos no hay nada
 * que aislar por componente, así que crear uno por vista solo gastaría memoria.
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

  // Ganchos del renderer de animaciones del navegador. Aquí no hay nada que
  // agrupar ni que esperar: el core ya agrupa por frame.
  begin(): void {}
  end(): void {}
  whenRenderingDone(): Promise<unknown> {
    return Promise.resolve(null)
  }
}
