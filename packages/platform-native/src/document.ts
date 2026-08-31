/**
 * `DOCUMENT` de mentira.
 *
 * Angular pide este token en sitios que no tienen nada que ver con el DOM
 * (sanitizador, `ApplicationRef`, errores de arranque). No se puede quitar,
 * pero sí se puede dejar inerte: cada método devuelve lo mínimo para no
 * romper, y nada de esto alcanza jamás a una vista nativa.
 *
 * Si algún día un método de estos se llama de verdad y devuelve basura, el
 * fallo hay que arreglarlo en la plataforma, no aquí.
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
