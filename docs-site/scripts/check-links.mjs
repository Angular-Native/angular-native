// Every internal link points at a page that exists.
//
// Starlight does not check this. A `](/guide/how-it-works/)` aimed at nothing
// builds without a murmur and serves a 404, so the two most followed links on
// the site — the home page's second button and the quick start's last card —
// were dead for a while and nothing said so. This is the check that was
// missing.
//
// It reads the built site rather than the sources: what a page resolves to is
// Astro's business, and comparing against `dist/` asks the question the reader
// asks. Run it after `astro build`.
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative, sep } from 'node:path'

const DIST = 'dist'
const SOURCES = 'src/content/docs'

function walk(dir, match) {
  const out = []
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry)
    if (statSync(path).isDirectory()) out.push(...walk(path, match))
    else if (match(entry)) out.push(path)
  }
  return out
}

const pages = new Set(['/'])
for (const file of walk(DIST, (name) => name === 'index.html')) {
  const dir = relative(DIST, file).split(sep).slice(0, -1).join('/')
  pages.add(dir === '' ? '/' : `/${dir}/`)
}

// Both link spellings a page can carry: Markdown's, and an href in a component.
const LINK = /\]\((\/[^)\s]*)\)|href="(\/[^"]*)"/g

const broken = []
for (const file of walk(SOURCES, (name) => name.endsWith('.md') || name.endsWith('.mdx'))) {
  const text = readFileSync(file, 'utf8')
  for (const match of text.matchAll(LINK)) {
    // The fragment is the browser's business; only the page has to exist.
    let target = (match[1] ?? match[2]).split('#')[0]
    if (!target.endsWith('/')) target += '/'
    // Everything under `public/` is copied across verbatim and has no page.
    if (target.startsWith('/img/')) continue
    if (!pages.has(target)) broken.push(`${file} -> ${match[1] ?? match[2]}`)
  }
}

if (broken.length > 0) {
  console.error(`  FAIL ${broken.length} internal links point at nothing:`)
  for (const line of broken) console.error(`    ${line}`)
  process.exit(1)
}
console.log(`  ok   every internal link resolves, across ${pages.size} pages`)
