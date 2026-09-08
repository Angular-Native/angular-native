#!/usr/bin/env node
// The `an` on the PATH after `npm install -g @angular-native/cli`.
//
// Two things are shipped separately and have to find each other here.
//
//   · The **executable** is one native binary per host, in a package of its own
//     with `os` and `cpu` set, listed in `optionalDependencies`. npm installs
//     the one that matches and skips the other four, which is the arrangement
//     esbuild and swc use and the reason `npm i -g` downloads about five
//     megabytes instead of twenty-five.
//   · The **payload** —`crates/`, `shells/`, `packages/`, `scripts/bundle.mjs`—
//     is this package, and it is the same on every host, because what it
//     carries is Swift, Java and Rust *sources*, not anything compiled.
//
// Node's resolver is the only thing that knows where the manager put either of
// them: under npm, bun and yarn the two packages are siblings under
// `node_modules/@angular-native`, and under pnpm they are unrelated directories
// inside `node_modules/.pnpm` with no path from one to the other. So the
// resolution happens here, in JavaScript, and the answer is handed to the binary
// in `AN_HOME` — the same variable somebody with a git checkout would set by
// hand.
import { spawnSync } from 'node:child_process'
import { createRequire } from 'node:module'
import { dirname } from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

const payload = dirname(dirname(fileURLToPath(import.meta.url)))
const require = createRequire(import.meta.url)

// The five hosts a binary is published for. A platform that is not on the list
// is not a platform the binary was cross-compiled to, and saying so is better
// than a `spawn ENOENT` about a path nobody recognises.
const HOSTS = new Set([
  'darwin-arm64',
  'darwin-x64',
  'linux-arm64',
  'linux-x64',
  'win32-x64'
])

const host = `${process.platform}-${process.arch}`
const pkg = `@angular-native/cli-${host}`
const exe = process.platform === 'win32' ? 'an.exe' : 'an'

function fail(message) {
  process.stderr.write(`an: ${message}\n`)
  process.exit(1)
}

if (!HOSTS.has(host)) {
  fail(
    `there is no prebuilt \`an\` for ${host}.\n` +
      '    The published hosts are ' +
      [...HOSTS].join(', ') +
      '.\n' +
      '    On any other one the CLI still builds from source:\n' +
      '      cargo install --git https://github.com/Angular-Native/angular-native an-cli'
  )
}

let binary
try {
  binary = require.resolve(`${pkg}/bin/${exe}`)
} catch {
  fail(
    `${pkg} is not installed, and it is the package that carries the executable for this machine.\n` +
      '    It is an optional dependency, so a failed download during `npm install` is silent.\n' +
      `    Reinstalling is the fix:  npm install -g @angular-native/cli --force`
  )
}

// `AN_HOME` is not overwritten. Somebody who has both this package and a
// checkout, and set the variable, meant the checkout: it is how the framework
// itself gets worked on with an `an` that came from npm.
const env = { ...process.env }
env.AN_HOME ??= payload

const { error, status, signal } = spawnSync(binary, process.argv.slice(2), {
  stdio: 'inherit',
  env
})
if (error) {
  fail(`${binary} could not be run: ${error.message}`)
}
// A binary killed by a signal has no exit code, and exiting 0 there would tell
// a CI job that an interrupted build succeeded.
process.exit(signal ? 1 : (status ?? 1))
