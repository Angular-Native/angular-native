// Bundling with esbuild plus the Angular Linker.
//
// The Angular packages are published in "partial" mode: their decorators are
// left as `ɵɵngDeclare*` calls that somebody has to resolve. The Angular CLI
// does it with a Babel plugin; without it, the first `@angular/common` class to
// be instantiated asks for the compiler at runtime, which is exactly what this
// project does not take to the device.
import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import process from 'node:process'

import { ConsoleLogger, LogLevel, NodeJSFileSystem } from '@angular/compiler-cli'
import { createEs2015LinkerPlugin } from '@angular/compiler-cli/linker/babel'
import { createRequire } from 'node:module'
import * as esbuild from 'esbuild'

// @babel/core is CommonJS and does not expose `default` to ESM.
const babel = createRequire(import.meta.url)('@babel/core')

const [entry, outfile, ...flags] = process.argv.slice(2)
if (!entry || !outfile) {
  console.error('usage: bundle.mjs <entry.js> <output.js> [--release] [--alias=k=v]')
  process.exit(2)
}
const release = flags.includes('--release')
const alias = Object.fromEntries(
  flags
    .filter((flag) => flag.startsWith('--alias='))
    .map((flag) => flag.slice('--alias='.length))
    .map((pair) => {
      const at = pair.indexOf('=')
      return [pair.slice(0, at), pair.slice(at + 1)]
    })
)

const linker = createEs2015LinkerPlugin({
  fileSystem: new NodeJSFileSystem(),
  logger: new ConsoleLogger(LogLevel.warn),
  linkerJitMode: false,
  // Without this the linker leaves the templates to the runtime compiler.
  sourceMapping: false
})

const angularLinker = {
  name: 'angular-linker',
  setup(build) {
    build.onLoad({ filter: /\.m?js$/ }, async (args) => {
      const source = await readFile(args.path, 'utf8')
      // Only the files that carry partial declarations go through Babel:
      // passing the whole tree would multiply the build time by ten.
      if (!source.includes('ɵɵngDeclare')) return null
      const result = await babel.transformAsync(source, {
        filename: args.path,
        babelrc: false,
        configFile: false,
        compact: false,
        browserslistConfigFile: false,
        plugins: [linker]
      })
      return { contents: result.code, loader: 'js' }
    })
  }
}

/** What the two halves and the production build have in common. */
const common = {
  bundle: true,
  platform: 'neutral',
  target: 'es2022',
  // `es2015` before `module`: under `module` rxjs publishes an ES5 build
  // transpiled with tslib helpers, and that is where QuickJS chokes. It is the
  // same preference the Angular CLI applies.
  mainFields: ['es2015', 'module', 'main'],
  conditions: ['es2015', 'module'],
  logLevel: 'warning',
  plugins: [angularLinker]
}

if (release) {
  await esbuild.build({
    ...common,
    entryPoints: [entry],
    outfile,
    format: 'iife',
    alias,
    minify: true,
    // `ngDevMode` set to false removes Angular's development checks, which are
    // almost half the bundle.
    define: { ngDevMode: 'false', ngJitMode: 'false' }
  })
} else {
  await writeFile(outfile, await split())
}

/**
 * The development bundle, split into two halves inside a single file.
 *
 * On top goes what does not change while you are programming —Angular, rxjs and
 * the framework packages, among them the one that keeps the node counter and the
 * command buffer—, wrapped in an `if` that is only entered the first time. Below
 * goes the app's code, in a module that can be evaluated again on top of the one
 * already running.
 *
 * That is the whole condition for hot reload to work: if the whole of Angular
 * were re-evaluated on reloading, there would be two copies in the interpreter,
 * and the one that knows which views are mounted would be the old one. Changing
 * the components in the new copy would move nothing on screen.
 */
async function split() {
  // The app half is bundled first: the list of what has to go into the other
  // one comes out of it.
  const shared = new Set()
  const externalize = {
    name: 'externalize',
    setup(build) {
      // Anything that is not a relative path is a package, and every package
      // goes on top. The primitives included: `platform-native` depends on them
      // —`NativeStack` mounts a `StackView`—, so they cannot be separated.
      // Touching a primitive causes a full reload, which is right: it is
      // framework code, not app code.
      build.onResolve({ filter: /^[^./]/ }, (args) => {
        shared.add(args.path)
        return { path: args.path, external: true }
      })
    }
  }

  const app = await esbuild.build({
    ...common,
    entryPoints: [entry],
    write: false,
    format: 'cjs',
    plugins: [...common.plugins, externalize]
  })

  const ids = [...shared].sort()
  const imports = ids.map((id, i) => `import * as m${i} from ${JSON.stringify(id)}`).join('\n')
  const table = ids.map((id, i) => `  [${JSON.stringify(id)}]: m${i}`).join(',\n')
  const vendorEntry = `${imports}
globalThis.__anModules = {
${table}
}
globalThis.__anRequire = (id) => {
  const mod = globalThis.__anModules[id]
  if (!mod) {
    throw new Error('angular-native: the bundle does not carry the module ' + id)
  }
  return mod
}
`

  const vendor = await esbuild.build({
    ...common,
    stdin: { contents: vendorEntry, resolveDir: process.cwd(), loader: 'js' },
    write: false,
    format: 'iife',
    alias
  })

  const vendorCode = vendor.outputFiles[0].text
  // The signature says which top half is loaded. If it does not match on
  // reloading —`platform-native` was touched, or a dependency—, the bottom half
  // is not evaluated: asking for a full restart is the only honest thing,
  // because the new code on top cannot get into an interpreter that already has
  // the old one.
  const stamp = createHash('sha256').update(vendorCode).digest('hex').slice(0, 16)

  return `// development bundle: shared half + reloadable half
if (!globalThis.__anModules) {
${vendorCode}
globalThis.__anVendor = ${JSON.stringify(stamp)}
}
if (globalThis.__anVendor !== ${JSON.stringify(stamp)}) {
  globalThis.__anHotOk = false
} else {
  ;(function (require, module, exports) {
${app.outputFiles[0].text}
  })(globalThis.__anRequire, { exports: {} }, {})
}
`
}
