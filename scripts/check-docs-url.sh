#!/usr/bin/env bash
# That everything pointing at the documentation points at the same place.
#
# The site's address is written out in four languages: Markdown links in the
# README, `homepage` fields in the package manifests, Rust string literals in
# the messages the CLI prints when it wants to send somebody to a page, and a
# Javadoc comment in a plugin. None of them can read a variable from the others,
# so the address is duplicated — and a stale one is worse than no link at all,
# because it takes the reader somewhere that is not ours.
#
# `DOCS_URL` at the root is the one place it is decided. Astro reads that file
# directly, and derives the `CNAME` from it; everything else is checked against
# it here and rewritten by `--fix`. Moving the site is: edit `DOCS_URL`, run
# this with `--fix`.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

python3 - "$ROOT" "${1:-}" <<'PY'
import pathlib, re, sys

root, flag = pathlib.Path(sys.argv[1]), sys.argv[2]
fix = flag == '--fix'
if flag and not fix:
    print(f"  FAIL unknown argument {flag!r}; the only one is --fix")
    sys.exit(1)

canonical = (root / 'DOCS_URL').read_text().strip()
bare = canonical.split('//', 1)[1]

# The project's own host, with or without the scheme — one shell check asserts
# on the bare hostname inside a message. `github.com/Angular-Native` is a
# different thing and keeps its capital A, so this never reaches it.
host = re.compile(r'(?<![\w/.-])(https://)?angular-native\.[A-Za-z0-9.-]+')


def is_site(match, text):
    # `angular-native.json` and `angular-native.entitlements` are file names the
    # CLI writes, and they look exactly like a bare host. What tells them apart
    # is that an address is either introduced by its scheme or followed by the
    # path it points at.
    return bool(match.group(1)) or text[match.end():match.end() + 1] == '/'


def replace(match, text):
    if not is_site(match, text):
        return match.group(0)
    return canonical if match.group(1) else bare


# Generated, vendored and installed trees are not sources of truth, and
# `DOCS_URL` itself is the answer rather than a copy of it.
skip = {'.git', 'node_modules', 'build', 'vendor', 'target', 'dist',
        '.astro', '.angular-native', 'DOCS_URL'}

stale, seen = [], 0
for path in sorted(root.rglob('*')):
    rel = path.relative_to(root)
    if skip.intersection(rel.parts) or path.is_symlink() or not path.is_file():
        continue
    try:
        text = path.read_text()
    except (UnicodeDecodeError, OSError):
        continue
    matches = [m for m in host.finditer(text) if is_site(m, text)]
    if not matches:
        continue
    seen += len(matches)
    wrong = [m.group(0) for m in matches if m.group(0) not in (canonical, bare)]
    if not wrong:
        continue
    if fix:
        path.write_text(host.sub(lambda m: replace(m, text), text))
    else:
        stale.append((rel.as_posix(), sorted(set(wrong))))

if stale:
    for name, urls in stale:
        print(f"  FAIL {name} points at {', '.join(urls)}")
    print(f"  (DOCS_URL says {canonical}; run scripts/check-docs-url.sh --fix)")
    sys.exit(1)

verb = 'now point' if fix else 'point'
print(f"  ok   the {seen} documentation links {verb} at {canonical}")
PY
