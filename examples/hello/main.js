'use strict'
// La misma pantalla que construía `build_demo` en Rust, ahora desde JS.
//
// No hay Angular todavía: esto llama a `__an_dom` a pelo, que es exactamente
// lo que hará el `Renderer2` de la fase siguiente. Si esto se ve en pantalla,
// la cadena JS -> búfer -> shadow tree -> taffy -> UIKit está entera.

const dom = globalThis.__an_dom

function el(kind, style, props) {
  const id = dom.createNode(kind)
  for (const name in style || {}) dom.setStyle(id, name, String(style[name]))
  for (const key in props || {}) dom.setProp(id, key, props[key])
  return id
}

function text(value) {
  const id = dom.createNode('RawText')
  dom.setText(id, value)
  return id
}

function append(parent, children) {
  for (let i = 0; i < children.length; i++) dom.insertChild(parent, children[i], i)
}

const root = el(
  'View',
  { width: '100%', height: '100%', paddingTop: 64, paddingHorizontal: 16, gap: 16 },
  { backgroundColor: '#0b1020' }
)
dom.setRoot(root)

const title = el('Text', {}, { fontSize: 28, fontWeight: 'bold', color: '#f4f7ff' })
append(title, [text('angular-native')])

const row = el('View', { flexDirection: 'row', gap: 12 })
const cards = [
  el('View', { flexGrow: 1, height: 88 }, { backgroundColor: '#1e2a4a', borderRadius: 12 }),
  el('View', { flexGrow: 2, height: 88 }, { backgroundColor: '#2b1e4a', borderRadius: 12 })
]
append(row, cards)

const paragraph = el('Text', {}, { fontSize: 16, color: '#9fb0d4' })
append(paragraph, [
  text(
    'Este párrafo lo mide UIKit y lo coloca taffy. Ninguna vista de esta ' +
      'pantalla es un WebView: son UIView, UILabel y nada más.'
  )
])

// Un contador con setInterval: prueba que los temporizadores corren, que un
// cambio de texto reflota el layout, y que el frame siguiente solo reemite
// los nodos afectados.
const counterLabel = el('Text', {}, { fontSize: 16, color: '#6ee7b7' })
const counterText = text('frames: 0')
append(counterLabel, [counterText])

append(root, [title, row, paragraph, counterLabel])

let seconds = 0
setInterval(() => {
  seconds++
  dom.setText(counterText, `segundos en marcha: ${seconds}`)
}, 1000)

console.log('main.js montado')
