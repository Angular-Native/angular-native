/**
 * Árbol de nodos del lado JS.
 *
 * El árbol autoritativo vive en Rust, pero `Renderer2` exige `parentNode()` y
 * `nextSibling()`, y `insertBefore()` recibe un nodo de referencia, no un
 * índice. Preguntar eso al core en cada llamada sería un ida y vuelta síncrono
 * por mutación — justo lo que el búfer de comandos evita. Así que este lado
 * mantiene la topología, y solo manda mutaciones.
 *
 * Es también donde se decide qué nodos existen de verdad. El host de un
 * componente Angular —`<app-root>`, `<VirtualList>`, `<page-home>`— no es una
 * vista: es un envoltorio. Si nadie le pone estilo, prop ni oyente, no se crea
 * en el core y sus hijos cuelgan del abuelo. Fabric hace lo mismo en una pasada
 * posterior y lo llama *view flattening*; aquí sale gratis porque este lado ve
 * la secuencia entera antes de mandarla.
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

export type NativeKind =
  | 'View'
  | 'Text'
  | 'RawText'
  | 'Image'
  | 'ScrollView'
  | 'TextInput'
  | 'StackView'
  | 'TabBar'
  | 'Switch'
  | 'Slider'
  | 'ActivityIndicator'
  | 'ProgressBar'
  | 'Button'
  | 'Modal'
  | 'Alert'
  | 'Icon'

/** Primitivas que el core sabe montar. El resto son hosts de componentes. */
const KINDS: Record<string, NativeKind> = {
  View: 'View',
  Text: 'Text',
  Image: 'Image',
  ScrollView: 'ScrollView',
  TextInput: 'TextInput',
  StackView: 'StackView',
  TabBar: 'TabBar',
  Switch: 'Switch',
  Slider: 'Slider',
  ActivityIndicator: 'ActivityIndicator',
  ProgressBar: 'ProgressBar',
  Button: 'Button',
  Modal: 'Modal',
  Alert: 'Alert',
  Icon: 'Icon'
}

export class NativeNode {
  parent: NativeNode | null = null
  readonly children: NativeNode[] = []
  /**
   * Un nodo destruido no vuelve. Angular sigue soltando suscripciones después
   * de destruir la vista —un `(press)` desengancha su gesto al morir— y esas
   * operaciones llegarían al core apuntando a algo que ya no existe.
   */
  destroyed = false

  /**
   * `0` mientras el nodo no exista en el core. Los envoltorios y los
   * comentarios ancla de `@if` y `@for` viven solo en este lado.
   */
  id = 0

  constructor(
    readonly kind: NativeKind | 'Comment',
    /** `false` para envoltorios y comentarios: no hay vista nativa detrás. */
    public materialized: boolean
  ) {}

  /** Tiene vista nativa propia ahora mismo. */
  get mounted(): boolean {
    return this.materialized && !this.destroyed
  }

  indexOf(child: NativeNode): number {
    return this.children.indexOf(child)
  }
}

/** Cuántas vistas nativas cuelgan de aquí, contándose a sí mismo. */
function mountedCount(node: NativeNode): number {
  if (node.mounted) return 1
  let total = 0
  for (const child of node.children) total += mountedCount(child)
  return total
}

/** Las vistas nativas más altas que cuelgan de aquí, en orden. */
function mountedRoots(node: NativeNode, out: NativeNode[] = []): NativeNode[] {
  if (node.mounted) {
    out.push(node)
    return out
  }
  for (const child of node.children) mountedRoots(child, out)
  return out
}

/** El ancestro que sí tiene vista nativa, saltándose los envoltorios. */
function mountParent(node: NativeNode): NativeNode | null {
  let parent = node.parent
  while (parent && !parent.mounted) parent = parent.parent
  return parent
}

/**
 * Posición que le toca a este nodo dentro de su padre montado.
 *
 * Hay que contar hacia arriba: entre él y su padre real puede haber varios
 * envoltorios, y cada uno aporta las vistas de sus hermanos anteriores.
 */
function mountIndex(node: NativeNode): number {
  let index = 0
  let current: NativeNode = node
  while (current.parent) {
    const parent: NativeNode = current.parent
    for (const sibling of parent.children) {
      if (sibling === current) break
      index += mountedCount(sibling)
    }
    if (parent.mounted) break
    current = parent
  }
  return index
}

export function createElementNode(name: string): NativeNode {
  const kind = KINDS[name]
  if (kind) {
    const node = new NativeNode(kind, true)
    node.id = __an_dom.createNode(kind)
    return node
  }
  // Host de un componente: por ahora no existe. Si alguien le pone algo
  // encima, se creará entonces.
  return new NativeNode('View', false)
}

export function createTextNode(value: string): NativeNode {
  const node = new NativeNode('RawText', true)
  node.id = __an_dom.createNode('RawText')
  if (value !== '') __an_dom.setText(node.id, value)
  return node
}

/**
 * Un comentario no tiene contrapartida nativa: es solo un marcador de posición
 * de `@if` y `@for` en el árbol de JS.
 */
export function createCommentNode(): NativeNode {
  return new NativeNode('Comment', false)
}

/**
 * Crea de verdad un envoltorio que hasta ahora no existía.
 *
 * Ocurre cuando algo le pone un estilo, una prop o un oyente: deja de ser un
 * envoltorio y pasa a ser una vista. Sus hijos, que colgaban del abuelo, se
 * mudan dentro.
 */
export function materialize(node: NativeNode): void {
  if (node.materialized || node.destroyed || node.kind === 'Comment') return

  // Se apuntan antes de cambiar nada: después de materializar, `mountedRoots`
  // devolvería el propio nodo.
  const descendants = mountedRoots(node)
  const parent = mountParent(node)
  const index = mountIndex(node)

  node.id = __an_dom.createNode(node.kind as NativeKind)
  node.materialized = true

  // Los hijos se sacan del abuelo y se meten dentro, en el mismo orden.
  for (const child of descendants) {
    const previous = mountParent(child)
    if (previous && previous !== node) __an_dom.removeChild(previous.id, child.id)
  }
  if (parent) __an_dom.insertChild(parent.id, node.id, index)
  descendants.forEach((child, position) => {
    __an_dom.insertChild(node.id, child.id, position)
  })
}

export function attach(parent: NativeNode, child: NativeNode, index: number): void {
  if (child.destroyed || parent.destroyed) return
  if (child.parent) detach(child.parent, child)
  parent.children.splice(index, 0, child)
  child.parent = parent

  const target = mountParent(child)
  if (!target) return
  // Si el que entra es un envoltorio, lo que se monta son las vistas que
  // cuelgan de él.
  let position = mountIndex(child)
  for (const view of mountedRoots(child)) {
    __an_dom.insertChild(target.id, view.id, position++)
  }
}

export function detach(parent: NativeNode, child: NativeNode): void {
  const at = parent.children.indexOf(child)
  if (at === -1) return

  const target = mountParent(child)
  const views = mountedRoots(child)
  parent.children.splice(at, 1)
  child.parent = null

  if (!target || target.destroyed) return
  for (const view of views) {
    if (!view.destroyed) __an_dom.removeChild(target.id, view.id)
  }
}

/**
 * Marca el nodo y todo lo que cuelga de él.
 *
 * El core destruye subárboles enteros de una vez, así que basta con que
 * Angular pida borrar el padre para que los hijos dejen de existir allí. Si
 * este lado no se entera, la baja de un `(press)` de un hijo llega después
 * apuntando a un nodo que ya no está.
 */
export function markDestroyed(node: NativeNode): void {
  if (node.destroyed) return
  node.destroyed = true
  for (const child of node.children) markDestroyed(child)
}

export const dom = __an_dom
