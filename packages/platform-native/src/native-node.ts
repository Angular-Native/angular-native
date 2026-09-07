/**
 * The node tree on the JS side.
 *
 * The authoritative tree lives in Rust, but `Renderer2` demands `parentNode()`
 * and `nextSibling()`, and `insertBefore()` takes a reference node, not an
 * index. Asking the core for that on every call would be a synchronous round
 * trip per mutation — precisely what the command buffer avoids. So this side
 * keeps the topology, and only sends mutations.
 *
 * It is also where it is decided which nodes really exist. An Angular
 * component's host —`<app-root>`, `<an-virtual-list>`, `<page-home>`— is not a
 * view: it is a wrapper. If nobody puts a style, a prop or a listener on it, it
 * never gets created in the core and its children hang off the grandparent.
 * Fabric does the same thing in a later pass and calls it *view flattening*;
 * here it comes free, because this side sees the whole sequence before sending
 * it.
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

/**
 * The core's vocabulary: the primitives it knows how to mount.
 *
 * It is a list and not a tag-to-primitive map because the tag is translated by a
 * rule, not by a table: `an-text-input` is `TextInput` and `an-view` is `View`.
 * Adding a primitive means writing its name here and its directive in
 * `packages/primitives`; there is no third list that can fall behind without
 * anybody noticing, and what is left is watched by `scripts/check-kinds.sh`.
 *
 * `RawText` is not here: it has no tag because no template ever writes it,
 * `Renderer2.createText()` creates it.
 */
// `Custom` at the end is the hole a plugin view comes through: the kind says
// "ask the host plugin-view registry" and the `an:view` prop says which one.
const NATIVE_KINDS = [
  'View',
  'Text',
  'Image',
  'ScrollView',
  'TextInput',
  'StackView',
  'TabBar',
  'Switch',
  'Slider',
  'ActivityIndicator',
  'ProgressBar',
  'Button',
  'Modal',
  'Alert',
  'Icon',
  'SegmentedControl',
  'Stepper',
  'SearchBar',
  'Select',
  'DatePicker',
  'NavigationBar',
  'Textarea',
  'WebView',
  'MapView',
  'VideoView',
  'Custom'
] as const

export type NativeKind = (typeof NATIVE_KINDS)[number] | 'RawText'

const MOUNTABLE = new Set<string>(NATIVE_KINDS)

/** The prefix on every one of the framework's tags, Ionic-style. */
const PREFIX = 'an-'

/**
 * From the tag to the primitive's name: strip `an-` and join the pieces in
 * PascalCase.
 *
 * It is a rule and not a table on purpose. The prefix is also what tells a
 * primitive apart from a component's host: `<app-root>` and `<page-home>` do not
 * carry it, which is why they are not looked up here. A misspelt `an-` cannot
 * sneak through —Angular rejects at compile time any tag that does not match a
 * directive— so here it is enough not to recognise it and treat it as a wrapper.
 */
function kindFromTag(tag: string): NativeKind | null {
  if (!tag.startsWith(PREFIX)) return null
  const kind = tag
    .slice(PREFIX.length)
    .replace(/(^|-)([a-z])/g, (_, __, letter: string) => letter.toUpperCase())
  return MOUNTABLE.has(kind) ? (kind as NativeKind) : null
}

export class NativeNode {
  parent: NativeNode | null = null
  readonly children: NativeNode[] = []
  /**
   * A destroyed node never comes back. Angular goes on dropping subscriptions
   * after destroying the view —a `(press)` unhooks its gesture as it dies— and
   * those operations would reach the core pointing at something that is no
   * longer there.
   */
  destroyed = false

  /**
   * `0` for as long as the node does not exist in the core. Wrappers and the
   * anchor comments of `@if` and `@for` live on this side only.
   */
  id = 0

  constructor(
    readonly kind: NativeKind | 'Comment',
    /** `false` for wrappers and comments: there is no native view behind. */
    public materialized: boolean
  ) {}

  /** It has a native view of its own right now. */
  get mounted(): boolean {
    return this.materialized && !this.destroyed
  }

  indexOf(child: NativeNode): number {
    return this.children.indexOf(child)
  }
}

/** How many native views hang off here, counting itself. */
function mountedCount(node: NativeNode): number {
  if (node.mounted) return 1
  let total = 0
  for (const child of node.children) total += mountedCount(child)
  return total
}

/** The topmost native views hanging off here, in order. */
function mountedRoots(node: NativeNode, out: NativeNode[] = []): NativeNode[] {
  if (node.mounted) {
    out.push(node)
    return out
  }
  for (const child of node.children) mountedRoots(child, out)
  return out
}

/** The nearest ancestor that does have a native view, skipping wrappers. */
function mountParent(node: NativeNode): NativeNode | null {
  let parent = node.parent
  while (parent && !parent.mounted) parent = parent.parent
  return parent
}

/**
 * The position this node is due inside its mounted parent.
 *
 * The counting has to go upwards: between it and its real parent there may be
 * several wrappers, and each one contributes the views of the siblings before
 * it.
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
  const kind = kindFromTag(name)
  if (kind) {
    const node = new NativeNode(kind, true)
    node.id = __an_dom.createNode(kind)
    return node
  }
  // A component's host: for now it does not exist. If somebody puts something
  // on it, it will get created then.
  return new NativeNode('View', false)
}

export function createTextNode(value: string): NativeNode {
  const node = new NativeNode('RawText', true)
  node.id = __an_dom.createNode('RawText')
  if (value !== '') __an_dom.setText(node.id, value)
  return node
}

/**
 * A comment has no native counterpart: it is nothing but a placeholder for `@if`
 * and `@for` in the JS tree.
 */
export function createCommentNode(): NativeNode {
  return new NativeNode('Comment', false)
}

/**
 * Really creates a wrapper that until now did not exist.
 *
 * It happens when something puts a style, a prop or a listener on it: it stops
 * being a wrapper and becomes a view. Its children, which were hanging off the
 * grandparent, move inside it.
 */
export function materialize(node: NativeNode): void {
  if (node.materialized || node.destroyed || node.kind === 'Comment') return

  // They are written down before anything changes: after materialising,
  // `mountedRoots` would return the node itself.
  const descendants = mountedRoots(node)
  const parent = mountParent(node)
  const index = mountIndex(node)

  // The children come off the grandparent before this node becomes a view, and
  // not after: the moment `materialized` is `true`, every child's mounted parent
  // is already this node, so the detach was comparing itself against itself and
  // never got sent. The tree was left with the same child hanging off two
  // parents.
  //
  // Nobody noticed, because UIKit and AppKit move a view that already has a
  // parent without a word; Android's `ViewGroup.addView` throws, and the first
  // time anyone saw it was with an `an-safe-area` hot-reloaded on the watch: the
  // screen went black.
  //
  // Every mounted descendant hangs off the same place —this node's mounted
  // parent— because what sits in between is not a view.
  if (parent) {
    for (const child of descendants) __an_dom.removeChild(parent.id, child.id)
  }

  node.id = __an_dom.createNode(node.kind as NativeKind)
  node.materialized = true

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
  // If the one coming in is a wrapper, what gets mounted are the views hanging
  // off it.
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
 * Marks the node and everything hanging off it.
 *
 * The core destroys whole subtrees at once, so Angular asking for the parent to
 * be removed is enough for the children to stop existing over there. If this
 * side does not find out, a child's `(press)` unsubscribe arrives afterwards
 * pointing at a node that is no longer there.
 */
export function markDestroyed(node: NativeNode): void {
  if (node.destroyed) return
  node.destroyed = true
  for (const child of node.children) markDestroyed(child)
}

export const dom = __an_dom
