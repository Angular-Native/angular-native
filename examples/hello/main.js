'use strict'
// The same screen `build_demo` used to build in Rust, now from JS.
//
// There is no Angular yet: this calls `__an_dom` bare, which is exactly what the
// `Renderer2` of the next phase will do. If this shows up on screen, the chain
// JS -> buffer -> shadow tree -> taffy -> UIKit is complete.

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
    'This paragraph is measured by UIKit and placed by taffy. Not one view on ' +
      'this screen is a WebView: they are UIView, UILabel and nothing else.'
  )
])

// A counter with setInterval: it proves the timers run, that a text change
// reflows the layout, and that the next frame only re-emits the affected nodes.
const counterLabel = el('Text', {}, { fontSize: 16, color: '#6ee7b7' })
const counterText = text('frames: 0')
append(counterLabel, [counterText])

append(root, [title, row, paragraph, counterLabel])

let seconds = 0
setInterval(() => {
  seconds++
  dom.setText(counterText, `seconds running: ${seconds}`)
}, 1000)

console.log('main.js mounted')
