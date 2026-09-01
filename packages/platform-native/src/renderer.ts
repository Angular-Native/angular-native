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
 * `Renderer2` sobre primitivas nativas.
 *
 * Esta clase es toda la costura entre Angular y el core: las plantillas
 * compiladas por AOT emiten instrucciones que acaban aquí, así que una
 * plantilla Angular funciona sin tocarla — lo único que cambia es que los
 * elementos son `<View>` y `<Text>` en vez de `<div>` y `<span>`.
 */
/**
 * Props de texto que en CSS serían estilos y aquí no lo son.
 *
 * El tamaño y el peso de la letra no son estilo de caja: el layout los
 * necesita para medir y el host para dibujar, y los dos los leen de las props.
 * Sin esto, escribir `[style.fontSize]` en una plantilla —que Angular acepta
 * sin rechistar— no hacía nada: la letra se medía con la de por defecto y se
 * dibujaba con la de UIKit, que no es la misma, y el texto se salía de su
 * caja y lo recortaba el padre. Se veía como texto que desaparece.
 *
 * La forma recomendada sigue siendo la entrada tipada, `[fontSize]`, que el
 * compilador comprueba. Esto es para que la otra no mienta.
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
    // Angular normaliza los nombres de estilo a guiones antes de llegar aquí,
    // así que las dos grafías tienen que reconocerse: la que se escribe en la
    // plantilla y la que llega.
  ].flatMap((name) => {
    const dashed = name.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)
    return [
      [name, name],
      [dashed, name]
    ] as [string, string][]
  })
)

/**
 * Avisa una vez por nombre de estilo que nadie va a mirar.
 *
 * Angular deja escribir `[style.loQueSea]` sin rechistar, y hasta aquí eso
 * viajaba al host como una prop cualquiera: el host no la usaba y no pasaba
 * nada. Sin error y sin traza, que es lo que hace caros estos fallos —han
 * caído cuatro en un día, y los cuatro se veían como "esto no hace nada".
 *
 * Se avisa una vez por nombre: el mismo estilo se escribe en cada detección de
 * cambios, y avisar en todas llenaría el registro sin decir nada nuevo.
 */
const warned = new Set<string>()

function warnUnknownStyle(name: string): void {
  // El núcleo acepta las dos grafías, así que aquí también: si no, cada estilo
  // escrito en camello —que Angular entrega con guiones— daría un aviso falso.
  const camel = name.replace(/-([a-z])/g, (_, letter: string) => letter.toUpperCase())
  if (KNOWN_STYLES.has(camel) || TEXT_PROPS.has(camel) || warned.has(camel)) return
  warned.add(camel)
  console.warn(
    `[angular-native] nadie reconoce el estilo "${name}", así que no hará nada. ` +
      'Si es una propiedad del control —color, título, valor— va como entrada ' +
      'tipada, no como estilo.'
  )
}

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
    if (!this.writable(el)) return
    dom.setProp(el.id, name, value)
  }

  removeAttribute(el: NativeNode, name: string): void {
    if (!this.writable(el)) return
    dom.setProp(el.id, name, null)
  }

  /**
   * Deja el nodo listo para recibir algo.
   *
   * Un envoltorio de componente no existe en el core hasta que alguien le
   * pone estilo, prop u oyente: en ese momento deja de ser un envoltorio.
   */
  private writable(node: NativeNode): boolean {
    if (node.destroyed || node.kind === 'Comment') return false
    if (!node.materialized) materialize(node)
    return node.materialized
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
    if (!this.writable(el)) return
    dom.setProp(el.id, 'className', [...set].join(' '))
  }

  setStyle(el: NativeNode, style: string, value: unknown, flags?: RendererStyleFlags2): void {
    if (!this.writable(el)) return
    warnUnknownStyle(style)
    // `!important` no significa nada sin cascada: se ignora la bandera y se
    // aplica el valor, que es el único comportamiento posible aquí.
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
    if (!this.writable(target)) return () => {}
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
