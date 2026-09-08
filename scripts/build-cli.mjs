// Assembles what `npm install -g @angular-native/cli` actually downloads.
//
// The CLI is published as six packages, not one, for the reason esbuild and swc
// publish that way: the executable is per-host and the rest is not.
//
//   · `@angular-native/cli-<host>` — one native `an` and nothing else, with
//     `os` and `cpu` set so npm installs the single one that matches. There are
//     five of them and they are generated here, not committed: a directory in
//     `packages/` whose only content is three lines of JSON and a binary CI
//     produces is a directory that only ever drifts.
//   · `@angular-native/cli` — the shim that resolves the one above, plus the
//     **payload**: `crates/`, `shells/`, `packages/` and `scripts/bundle.mjs`,
//     laid out exactly as they are in this repository so that `validate_sdk`
//     asks the same four questions of a checkout and of a `node_modules`.
//
// What is deliberately *not* in the payload is a compiled Rust core. See
// `docs-site/.../guide/installing.md`: the archives are per-target rather than
// per-host, there are fourteen of them, and they weigh more than the Xcode and
// NDK toolchains they would be saving nobody from installing.
//
// Usage:  node scripts/build-cli.mjs [--pack] [--binary <host>=<path>]…
//
//   --pack                  also writes tarballs into build/npm-cli
//   --print-hosts           writes the host table as JSON and does nothing else.
//                           `release.yml` builds its matrix out of it, so the
//                           list of hosts lives in one place.
//   --binary darwin-arm64=… puts a built `an` into that host's package. Without
//                           it the host's package is still generated, with the
//                           manifest and no executable, which is what the
//                           release workflow fills in on each runner.

import { execFileSync } from 'node:child_process'
import { chmodSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
// Not `build/npm`: `build-packages.mjs --pack` owns that one and this script
// starts by emptying whatever directory it is given.
const out = join(root, 'build/npm-cli')

// host -> the Rust target triple its `an` is cross-compiled to, the npm `os`
// and `cpu` that keep it off every other machine, and the GitHub runner that
// builds it. The keys are `${process.platform}-${process.arch}`, which is what
// `bin/an.mjs` computes.
//
// Each one is a **native** build on its own runner rather than a cross-compile
// from one: `an-cli` links QuickJS through `cc`, and a cross-compiled C
// toolchain is a second thing that can be wrong about a release nobody can
// re-cut.
export const HOSTS = {
  'darwin-arm64': {
    target: 'aarch64-apple-darwin',
    os: 'darwin',
    cpu: 'arm64',
    runner: 'macos-14'
  },
  'darwin-x64': {
    target: 'x86_64-apple-darwin',
    os: 'darwin',
    cpu: 'x64',
    runner: 'macos-13'
  },
  'linux-arm64': {
    target: 'aarch64-unknown-linux-gnu',
    os: 'linux',
    cpu: 'arm64',
    runner: 'ubuntu-24.04-arm'
  },
  'linux-x64': {
    target: 'x86_64-unknown-linux-gnu',
    os: 'linux',
    cpu: 'x64',
    runner: 'ubuntu-latest'
  },
  'win32-x64': {
    target: 'x86_64-pc-windows-msvc',
    os: 'win32',
    cpu: 'x64',
    runner: 'windows-latest'
  }
}

// Everything the CLI reaches for through `workspace.root`, and nothing else.
// A path that is missing here does not fail at pack time: it fails on somebody
// else's machine, at `an ios`, as a `swiftc` with no sources. The `REQUIRED`
// check at the end of the assembly is what turns that into a failure here.
const PAYLOAD = [
  // The Rust core is shipped as source and compiled by the user's cargo. The
  // manifest and the lockfile come with it so the build is the one this release
  // was tested with, and `rust-toolchain.toml` so rustup installs the stable
  // toolchain and the four targets without being asked.
  'Cargo.toml',
  'Cargo.lock',
  'rust-toolchain.toml',
  'crates',
  // Swift for the five Apple shells, Java for the two Android ones. Not
  // optional and not compilable ahead of time: `an ios` hands these very files
  // to `swiftc` along with each plugin's own sources, in one invocation, which
  // is what lets a plugin see `AnPlugin` without importing anything.
  'shells',
  // The Angular Linker plus esbuild. Its own imports resolve against this
  // package's dependencies, which is why they are dependencies and not peers.
  'scripts/bundle.mjs',
  // Material and everything it drags along, resolved and with its resources
  // compiled. `an android` does not run these — it reads what they left in
  // `vendor/android/build` and, finding nothing, prints the command to run.
  // A payload without them prints a command that names a file it does not have.
  'scripts/fetch-android-deps.py',
  'scripts/prepare-android-deps.py',
  // `runtime.js` is evaluated by the engine before anything else, and the other
  // two are the TypeScript `an init` compiles with the *project's* ngc and packs
  // into `.angular-native/vendor`. Sources, therefore, not `dist`.
  'packages/runtime',
  'packages/primitives/package.json',
  'packages/primitives/src',
  'packages/platform-native/package.json',
  'packages/platform-native/src',
  'LICENSE'
]

// The four `validate_sdk` asks for, restated here so the tarball is checked
// against the same list the binary checks the unpacked directory against.
const REQUIRED = ['packages/runtime/runtime.js', 'scripts/bundle.mjs', 'shells', 'crates']

const argv = process.argv.slice(2)
if (argv.includes('--print-hosts')) {
  process.stdout.write(
    `${JSON.stringify(Object.entries(HOSTS).map(([host, rest]) => ({ host, ...rest })))}\n`
  )
  process.exit(0)
}
const pack = argv.includes('--pack')
const binaries = new Map()
for (let i = 0; i < argv.length; i++) {
  const flag = argv[i]
  if (flag !== '--binary' && !flag.startsWith('--binary=')) {
    if (flag !== '--pack') throw new Error(`unknown argument ${flag}`)
    continue
  }
  const pair = flag === '--binary' ? argv[++i] : flag.slice('--binary='.length)
  const at = pair?.indexOf('=') ?? -1
  if (at < 1) throw new Error(`--binary wants <host>=<path>, and was given ${pair}`)
  const host = pair.slice(0, at)
  if (!(host in HOSTS)) {
    throw new Error(`--binary names ${host}, which is not one of ${Object.keys(HOSTS).join(', ')}`)
  }
  binaries.set(host, resolve(pair.slice(at + 1)))
}

const manifest = JSON.parse(readFileSync(join(root, 'packages/cli/package.json'), 'utf8'))
const { version } = manifest

// One release, one version. `check-publish.sh` holds the framework packages and
// the workspace to it; the five generated ones are pinned by
// `optionalDependencies`, and a pin that does not match the version being built
// installs a CLI whose executable is from another release.
for (const host of Object.keys(HOSTS)) {
  const pinned = manifest.optionalDependencies?.[`@angular-native/cli-${host}`]
  if (pinned !== version) {
    throw new Error(
      `packages/cli/package.json pins @angular-native/cli-${host} at ${pinned}, and this build is ${version}`
    )
  }
}
for (const host of Object.keys(manifest.optionalDependencies ?? {})) {
  if (!(host.replace('@angular-native/cli-', '') in HOSTS)) {
    throw new Error(`packages/cli/package.json pins ${host}, which this script does not build`)
  }
}

rmSync(out, { recursive: true, force: true })
mkdirSync(out, { recursive: true })

const packed = []

// ── The five executables ────────────────────────────────────────────────────
for (const [host, { os, cpu }] of Object.entries(HOSTS)) {
  const dir = join(out, `cli-${host}`)
  mkdirSync(join(dir, 'bin'), { recursive: true })
  writeFileSync(
    join(dir, 'package.json'),
    `${JSON.stringify(
      {
        name: `@angular-native/cli-${host}`,
        version,
        description: `The \`an\` executable for ${host}. Installed by @angular-native/cli, which is the package to depend on.`,
        // No `main` and no `exports`: what the shim asks for is the subpath
        // `bin/an`, and an `exports` map would have to allow it explicitly.
        files: ['bin', 'README.md', 'LICENSE'],
        os: [os],
        cpu: [cpu],
        license: manifest.license,
        author: manifest.author,
        homepage: manifest.homepage,
        bugs: manifest.bugs,
        repository: manifest.repository,
        publishConfig: { access: 'public' }
      },
      null,
      2
    )}\n`
  )
  writeFileSync(
    join(dir, 'README.md'),
    `# @angular-native/cli-${host}\n\nThe \`an\` executable for ${host}, published on its own so that\n\`npm install -g @angular-native/cli\` downloads one host's binary and not five.\nInstall [\`@angular-native/cli\`](https://www.npmjs.com/package/@angular-native/cli)\ninstead of this.\n`
  )
  cpSync(join(root, 'LICENSE'), join(dir, 'LICENSE'))

  const built = binaries.get(host)
  if (built) {
    if (!existsSync(built)) throw new Error(`--binary ${host} points at ${built}, which is not there`)
    const name = os === 'win32' ? 'an.exe' : 'an'
    cpSync(built, join(dir, 'bin', name))
    chmodSync(join(dir, 'bin', name), 0o755)
    process.stderr.write(`==> cli-${host}: ${built}\n`)
  } else {
    process.stderr.write(`==> cli-${host}: manifest only, no executable\n`)
  }
  packed.push({ dir, name: `@angular-native/cli-${host}`, complete: Boolean(built) })
}

// ── The payload ─────────────────────────────────────────────────────────────
const cli = join(out, 'cli')
mkdirSync(cli, { recursive: true })
cpSync(join(root, 'packages/cli'), cli, { recursive: true })
for (const entry of PAYLOAD) {
  const from = join(root, entry)
  if (!existsSync(from)) throw new Error(`the payload wants ${entry}, and it is not in the repository`)
  const to = join(cli, entry)
  mkdirSync(dirname(to), { recursive: true })
  cpSync(from, to, { recursive: true })
}
for (const needed of REQUIRED) {
  if (!existsSync(join(cli, needed))) {
    throw new Error(`the payload has no ${needed}, and \`an\` refuses an SDK without it`)
  }
}
process.stderr.write(`==> ${manifest.name}: payload assembled in build/npm-cli/cli\n`)
packed.push({ dir: cli, name: manifest.name, complete: true })

if (pack) {
  for (const { dir, name } of packed) {
    const said = execFileSync('npm', ['pack', '--pack-destination', out], {
      cwd: dir,
      stdio: ['ignore', 'pipe', 'inherit'],
      encoding: 'utf8'
    })
    const file = said.trim().split('\n').filter(Boolean).at(-1)
    process.stderr.write(`==> packed ${name} -> build/npm-cli/${file}\n`)
  }
}

const missing = packed.filter((one) => !one.complete).map((one) => one.name)
if (missing.length) {
  process.stderr.write(`\n${packed.length} packages ready; no executable in: ${missing.join(', ')}\n`)
} else {
  process.stderr.write(`\n${packed.length} packages ready\n`)
}
