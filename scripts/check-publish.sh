#!/usr/bin/env bash
# That every package would arrive on npm as a package and not as a shell.
#
# The failure this exists to catch is quiet and expensive: `npm publish` is
# happy to ship a tarball with no code in it. A package whose `main` points at a
# TypeScript file nobody compiled, or whose `files` list forgot `dist`, installs
# cleanly and then fails at the consumer's first import — after it has a version
# number that can never be reused.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== npm packages"
fail=0

if [ ! -f LICENSE ]; then
  echo "  FAIL there is no LICENSE at the root, and every manifest claims MIT"
  fail=1
else
  echo "  ok   the licence the manifests claim is actually in the repository"
fi

node - "$ROOT" <<'NODE' || fail=1
const { readFileSync, existsSync } = require('node:fs')
const { join } = require('node:path')
const { execFileSync } = require('node:child_process')
const root = process.argv[2]

const shorts = execFileSync('ls', [join(root, 'packages')], { encoding: 'utf8' }).trim().split('\n')
let bad = 0
for (const short of shorts) {
  const dir = join(root, 'packages', short)
  const manifest = join(dir, 'package.json')
  if (!existsSync(manifest)) {
    console.log(`  FAIL packages/${short} has no package.json, so it can never be published`)
    bad++
    continue
  }
  const p = JSON.parse(readFileSync(manifest, 'utf8'))
  const problems = []
  if (p.private) problems.push('it is marked private')
  for (const field of ['version', 'description', 'license', 'repository', 'files']) {
    if (!p[field]) problems.push(`no ${field}`)
  }
  // What npm will really call `main` once it has rewritten publishConfig.
  const main = p.publishConfig?.main ?? p.main
  if (main && main.endsWith('.ts')) {
    problems.push(`main would ship as ${main}, which no consumer can run`)
  }
  if (main && !existsSync(join(dir, main))) {
    problems.push(`${main} does not exist — run scripts/build-packages.mjs`)
  }
  if (p.types && !existsSync(join(dir, p.types))) {
    problems.push(`${p.types} does not exist, so the package ships untyped`)
  }
  if (problems.length) {
    console.log(`  FAIL ${p.name || short}: ${problems.join('; ')}`)
    bad++
  } else {
    console.log(`  ok   ${p.name} ships ${main} with its types`)
  }
}
process.exit(bad ? 1 : 0)
NODE

if [ "$fail" -ne 0 ]; then
  exit 1
fi
