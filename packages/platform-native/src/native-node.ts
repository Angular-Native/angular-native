/**
 * Árbol de nodos del lado JS.
 *
 * El árbol autoritativo vive en Rust, pero `Renderer2` exige `parentNode()` y
 * `nextSibling()`, y `insertBefore()` recibe un nodo de referencia, no un
 * índice. Preguntar eso al core en cada llamada sería un ida y vuelta síncrono
 * por mutación — justo lo que el búfer de comandos evita. Así que este lado
 * mantiene la topología, y solo manda mutaciones.
 *
 * Es la misma decisión que toma Fabric con su shadow tree en JS.
 */

declare const __an_dom: {
  createNode(kind: string): number
  destroyNode(id: number): void
  insertChild(parent: number, child: number, index: number): void
  removeChild(parent: number, child: number): void
  setStyle(id: number, name: string, value: string): void
  setProp(id: number, key: string, value: unknown): void
  setText(id: number, text: string): void
  setRoot(id: number): void
  listen(id: number, event: string, handler: (payload: unknown) => void): () => void
}

export type NativeKind = 'View' | 'Text' | 'RawText' | 'Image' | 'ScrollView' | 'TextInput'

/** Primitivas que el core sabe montar. El resto son error de plantilla. */
const KINDS: Record<string, NativeKind> = {
  View: 'View',
  Text: 'Text',
  Image: 'Image',
  ScrollView: 'ScrollView',
  TextInput: 'TextInput'
}

export class NativeNode {
  parent: NativeNode | null = null
  readonly children: NativeNode[] = []

  constructor(
    readonly id: number,
    readonly kind: NativeKind | 'Comment'
  ) {}

  /** Índice del hijo dentro de este nodo, o -1. */
  indexOf(child: NativeNode): number {
    return this.children.indexOf(child)
  }

  /**
   * Los comentarios son anclas de `@if` y `@for`: existen en el árbol de JS
   * para poder calcular posiciones, pero nunca llegan al core.
   */
  get mounted(): boolean {
    return this.kind !== 'Comment'
  }

  /**
   * Índice que entiende el core: cuenta solo hermanos montables anteriores.
   * Sin esto, un `@if` que renderiza un comentario desplazaría a todos sus
   * hermanos una posición.
   */
  mountIndexOf(child: NativeNode): number {
    let index = 0
    for (const current of this.children) {
      if (current === child) return index
      if (current.mounted) index++
    }
    return index
  }
}

export function createElementNode(name: string): NativeNode {
  const kind = KINDS[name]
  if (!kind) {
    throw new Error(
      `<${name}> no es una primitiva nativa. Disponibles: ${Object.keys(KINDS).join(', ')}`
    )
  }
  return new NativeNode(__an_dom.createNode(kind), kind)
}

export function createTextNode(value: string): NativeNode {
  const node = new NativeNode(__an_dom.createNode('RawText'), 'RawText')
  if (value !== '') __an_dom.setText(node.id, value)
  return node
}

/**
 * Un comentario no tiene contrapartida nativa: es solo un marcador de posición
 * en el árbol de JS. Se le da un id negativo para que un uso accidental contra
 * el core falle ruidosamente en vez de corromper un nodo real.
 */
let nextCommentId = -1
export function createCommentNode(): NativeNode {
  return new NativeNode(nextCommentId--, 'Comment')
}

export function attach(parent: NativeNode, child: NativeNode, index: number): void {
  if (child.parent) detach(child.parent, child)
  parent.children.splice(index, 0, child)
  child.parent = parent
  if (child.mounted && parent.mounted) {
    __an_dom.insertChild(parent.id, child.id, parent.mountIndexOf(child))
  }
}

export function detach(parent: NativeNode, child: NativeNode): void {
  const at = parent.children.indexOf(child)
  if (at === -1) return
  parent.children.splice(at, 1)
  child.parent = null
  if (child.mounted && parent.mounted) {
    __an_dom.removeChild(parent.id, child.id)
  }
}

export const dom = __an_dom
