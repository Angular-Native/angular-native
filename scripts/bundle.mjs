// Empaquetado con esbuild más el Angular Linker.
//
// Los paquetes de Angular se publican en modo "partial": sus decoradores
// quedan como llamadas `ɵɵngDeclare*` que alguien tiene que resolver. El CLI
// de Angular lo hace con un plugin de Babel; sin él, la primera clase de
// `@angular/common` que se instancia pide el compilador en tiempo de
// ejecución, que es justo lo que este proyecto no lleva al dispositivo.
import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
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

/** Lo común a las dos mitades y a la build de producción. */
const common = {
  bundle: true,
  platform: 'neutral',
  target: 'es2022',
  // `es2015` antes que `module`: rxjs publica en `module` una build ES5
  // transpilada con helpers de tslib, y ahí es donde QuickJS se atraganta.
  // Es la misma preferencia que aplica el CLI de Angular.
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
    // `ngDevMode` a false quita las comprobaciones de desarrollo de Angular,
    // que son casi la mitad del bundle.
    define: { ngDevMode: 'false', ngJitMode: 'false' }
  })
} else {
  await writeFile(outfile, await split())
}

/**
 * El bundle de desarrollo, partido en dos mitades dentro de un mismo fichero.
 *
 * Arriba va lo que no cambia mientras se programa —Angular, rxjs y los
 * paquetes del framework, entre ellos el que guarda el contador de nodos y el
 * búfer de comandos—, envuelto en un `if` que solo entra la primera vez. Abajo
 * va el código de la app, en un módulo que puede volver a evaluarse encima del
 * que ya corre.
 *
 * Esa es toda la condición para que el refresco en caliente funcione: si al
 * recargar se reevaluara Angular entero, en el intérprete habría dos copias, y
 * la que sabe qué vistas hay montadas sería la vieja. Cambiar los componentes
 * en la copia nueva no movería nada en pantalla.
 */
async function split() {
  // La mitad de la app se empaqueta primero: de ahí sale la lista de lo que
  // hay que meter en la otra.
  const shared = new Set()
  const externalize = {
    name: 'externalize',
    setup(build) {
      // Todo lo que no sea una ruta relativa es un paquete, y todos los
      // paquetes van arriba. Las primitivas incluidas: `platform-native`
      // depende de ellas —`NativeStack` monta un `StackView`—, así que no se
      // pueden separar. Tocar una primitiva provoca recarga entera, que es lo
      // correcto: es código del framework, no de la app.
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
    throw new Error('angular-native: el bundle no trae el módulo ' + id)
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
  // La firma dice qué mitad de arriba está cargada. Si al recargar no coincide
  // —se tocó `platform-native`, o una dependencia—, la mitad de abajo no se
  // evalúa: pedir el reinicio entero es lo único honesto, porque el código
  // nuevo de arriba no puede entrar en un intérprete que ya tiene el viejo.
  const stamp = createHash('sha256').update(vendorCode).digest('hex').slice(0, 16)

  return `// bundle de desarrollo: mitad compartida + mitad recargable
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
