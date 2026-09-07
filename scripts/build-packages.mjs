// Compiles the npm packages into what actually gets published.
//
// Inside this repository the packages are consumed as TypeScript sources, through
// `paths` in each example's tsconfig — that is what makes editing the framework
// and an app in the same commit bearable. None of that can be published: a
// consumer needs compiled JavaScript and `.d.ts` files, and needs them built in
// Angular's **partial** compilation mode, where the decorators are left as
// `ɵɵngDeclare*` declarations for the Angular Linker to resolve. A library
// compiled in full mode is tied to the exact compiler version that built it.
//
// This is the same road `an init` takes when it vendors the framework into a
// project, and it is deliberately the same: two ways of building the packages
// would be two ways for them to differ.
//
// Usage:  node scripts/build-packages.mjs [--pack]
//         --pack also writes tarballs into build/npm, which is what you upload.

import { execFileSync } from 'node:child_process'
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync
} from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')

// In dependency order: a package is compiled against the `.d.ts` of the ones
// before it, so `primitives` cannot come after `platform`. The plugins are
// discovered rather than listed, because a list is a thing to forget — adding a
// plugin and not adding it here produced a package that published with no code
// in it, which `check-publish.sh` now catches but should not have to.
const ORDER = [
  'primitives',
  'platform-native',
  ...readdirSync(join(root, 'packages'))
    .filter((name) => name.startsWith('plugin-'))
    .sort()
]

const ALIASES = {
  '@angular-native/primitives': 'primitives',
  '@angular-native/platform': 'platform-native'
}

const pack = process.argv.includes('--pack')

function run(command, args, cwd = root) {
  return execFileSync(command, args, {
    cwd,
    stdio: ['ignore', 'pipe', 'inherit'],
    encoding: 'utf8'
  })
}

function tsconfigFor(short) {
  const dir = join(root, 'packages', short)
  // Every alias points at the `.d.ts` the previous package just emitted, so the
  // types a consumer will get are the types this build was checked against.
  const paths = Object.fromEntries(
    Object.entries(ALIASES)
      .filter(([, target]) => target !== short)
      .map(([name, target]) => [name, [join(root, 'packages', target, 'dist/public-api.d.ts')]])
  )
  return JSON.stringify(
    {
      compilerOptions: {
        target: 'es2022',
        module: 'esnext',
        moduleResolution: 'bundler',
        // `dom` is here for the types alone: Angular's own `.d.ts` files name
        // Document, Element and Event. None of the three exists at runtime.
        lib: ['es2022', 'dom'],
        strict: true,
        skipLibCheck: true,
        useDefineForClassFields: false,
        experimentalDecorators: false,
        moduleDetection: 'force',
        declaration: true,
        outDir: join(dir, 'dist'),
        rootDir: join(dir, 'src'),
        paths,
        types: []
      },
      files: [join(dir, 'src/public-api.ts')],
      angularCompilerOptions: { strictTemplates: true, compilationMode: 'partial' }
    },
    null,
    2
  )
}

const built = []
for (const short of ORDER) {
  const dir = join(root, 'packages', short)
  const name = JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8')).name
  process.stderr.write(`==> ${name}\n`)

  rmSync(join(dir, 'dist'), { recursive: true, force: true })
  const config = join(dir, '.tsconfig.build.json')
  writeFileSync(config, tsconfigFor(short))
  try {
    run('npx', ['--no-install', 'ngc', '-p', config])
  } finally {
    rmSync(config, { force: true })
  }

  const api = join(dir, 'dist/public-api.js')
  if (!existsSync(api)) {
    throw new Error(`ngc finished fine but left no ${api}`)
  }
  // Every package ships the licence it claims in its manifest.
  copyFileSync(join(root, 'LICENSE'), join(dir, 'LICENSE'))
  built.push({ short, dir, name })
}

// `runtime` is a plain script the engine evaluates before anything else. There
// is nothing to compile: it is published exactly as it is read.
copyFileSync(join(root, 'LICENSE'), join(root, 'packages/runtime/LICENSE'))
built.push({ short: 'runtime', dir: join(root, 'packages/runtime'), name: '@angular-native/runtime' })

if (pack) {
  const out = join(root, 'build/npm')
  rmSync(out, { recursive: true, force: true })
  mkdirSync(out, { recursive: true })
  for (const { dir, name } of built) {
    const said = run('npm', ['pack', '--pack-destination', out], dir)
    const file = said.trim().split('\n').filter(Boolean).at(-1)
    process.stderr.write(`==> packed ${name} -> build/npm/${file}\n`)
  }
}

process.stderr.write(`\n${built.length} packages ready\n`)
