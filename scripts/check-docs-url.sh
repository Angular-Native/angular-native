#!/usr/bin/env bash
# That everything pointing at the documentation points at the same place.
#
# The site's address is written out in four languages: Markdown links in the
# README, `homepage` fields in eight package manifests, Rust string literals in
# the messages the CLI prints when it wants to send somebody to a page, and a
# Javadoc comment in a plugin. None of them can read a variable from the other,
# so the address is duplicated — and a stale one is worse than no link at all,
# because it takes the reader somewhere that is not ours.
#
# `DOCS_URL` at the root is the one place it is decided. Astro reads that file
# directly; everything else is checked against it here, and rewritten by
# `--fix`. Moving the site is: edit `DOCS_URL`, run this with `--fix`.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

python3 - "$ROOT" "${1:-}" <<'PY'
import pathlib, re, subprocess, sys

root, flag = pathlib.Path(sys.argv[1]), sys.argv[2]
fix = flag == '--fix'
if flag and not fix:
    print(f"  FAIL unknown argument {flag!r}; the only one is --fix")
    sys.exit(1)

canonical = (root / 'DOCS_URL').read_text().strip()

# Only the project's own host. `github.com/Angular-Native` is a different thing
# and keeps its capital A, so the lowercase hostname does not reach it.
host = re.compile(r'https://angular-native\.[A-Za-z0-9.-]+')

# The generated and vendored trees are not sources of truth, and `DOCS_URL`
# itself is the answer rather than a copy of it.
skip = ('.git', 'node_modules', 'build', 'vendor', 'target', 'dist', '.astro')
files = subprocess.run(
    ['git', 'ls-files'], cwd=root, capture_output=True, text=True, check=True
).stdout.split()

stale, seen = [], 0
for name in files:
    if name == 'DOCS_URL' or any(part in skip for part in pathlib.Path(name).parts):
        continue
    path = root / name
    try:
        text = path.read_text()
    except (UnicodeDecodeError, FileNotFoundError):
        continue
    found = host.findall(text)
    if not found:
        continue
    seen += len(found)
    wrong = [url for url in found if url != canonical]
    if not wrong:
        continue
    if fix:
        path.write_text(host.sub(canonical, text))
    else:
        stale.append((name, sorted(set(wrong))))

if stale:
    for name, urls in stale:
        print(f"  FAIL {name} points at {', '.join(urls)}")
    print(f"  (DOCS_URL says {canonical}; run scripts/check-docs-url.sh --fix)")
    sys.exit(1)

verb = 'now point' if fix else 'point'
print(f"  ok   the {seen} documentation links {verb} at {canonical}")
PY
