/**
 * A pretend `DOCUMENT`.
 *
 * Angular asks for this token in places that have nothing to do with the DOM
 * (the sanitiser, `ApplicationRef`, bootstrap errors). It cannot be removed, but
 * it can be left inert: every method returns the bare minimum not to break
 * anything, and none of it ever reaches a native view.
 *
 * If one of these methods ever really gets called and returns rubbish, the bug
 * is to be fixed in the platform, not here.
 */
export function createFakeDocument(): Document {
  const noop = () => {}
  const element = {
    nodeType: 1,
    nodeName: 'BODY',
    style: {},
    attributes: {},
    childNodes: [] as unknown[],
    setAttribute: noop,
    removeAttribute: noop,
    getAttribute: () => null,
    appendChild: noop,
    removeChild: noop,
    addEventListener: noop,
    removeEventListener: noop,
    dispatchEvent: () => true
  }

  const doc = {
    nodeType: 9,
    nodeName: '#document',
    documentElement: element,
    body: element,
    head: element,
    defaultView: null,
    title: '',
    createElement: () => ({ ...element }),
    createElementNS: () => ({ ...element }),
    createTextNode: () => ({ nodeType: 3, nodeValue: '' }),
    createComment: () => ({ nodeType: 8, nodeValue: '' }),
    createDocumentFragment: () => ({ ...element }),
    querySelector: () => null,
    querySelectorAll: () => [],
    getElementById: () => null,
    getElementsByTagName: () => [],
    addEventListener: noop,
    removeEventListener: noop,
    dispatchEvent: () => true,
    createEvent: () => ({ initEvent: noop })
  }
  return doc as unknown as Document
}
