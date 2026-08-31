// Empaquetado con esbuild más el Angular Linker.
//
// Los paquetes de Angular se publican en modo "partial": sus decoradores
// quedan como llamadas `ɵɵngDeclare*` que alguien tiene que resolver. El CLI
// de Angular lo hace con un plugin de Babel; sin él, la primera clase de
// `@angular/common` que se instancia pide el compilador en tiempo de
// ejecución, que es justo lo que este proyecto no lleva al dispositivo.
import { readFile } from 'node:fs/promises'
import process from 'node:process'

import { ConsoleLogger, LogLevel, NodeJSFileSystem } from '@angular/compiler-cli'
import { createEs2015LinkerPlugin } from '@angular/compiler-cli/linker/babel'
import { createRequire } from 'node:module'
import * as esbuild from 'esbuild'

// @babel/core es CommonJS y no expone `default` a ESM.
const babel = createRequire(import.meta.url)('@babel/core')

const [entry, outfile, ...flags] = process.argv.slice(2)
if (!entry || !outfile) {
  console.error('uso: bundle.mjs <entrada.js> <salida.js> [--release] [--alias=k=v]')
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
  // Sin esto el linker deja las plantillas para el compilador de runtime.
  sourceMapping: false
})

const angularLinker = {
  name: 'angular-linker',
  setup(build) {
    build.onLoad({ filter: /\.m?js$/ }, async (args) => {
      const source = await readFile(args.path, 'utf8')
      // Solo pasan por Babel los ficheros que traen declaraciones parciales:
      // pasar el árbol entero multiplicaría el tiempo de build por diez.
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

await esbuild.build({
  entryPoints: [entry],
  outfile,
  bundle: true,
  format: 'iife',
  platform: 'neutral',
  target: 'es2022',
  mainFields: ['module', 'main'],
  conditions: ['module'],
  alias,
  minify: release,
  define: release ? { ngDevMode: 'false', ngJitMode: 'false' } : {},
  logLevel: 'warning',
  plugins: [angularLinker]
})
