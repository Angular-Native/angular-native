#!/usr/bin/env bash
# That the JS-side list of style names is still the core's.
#
# They are duplicated and it cannot be helped: the core needs it to resolve the
# layout and the renderer needs it to warn before sending something nobody is
# going to look at. What can be helped is their drifting apart without anybody
# noticing.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

python3 - "$ROOT" <<'PY'
import pathlib, re, sys

root = pathlib.Path(sys.argv[1])
source = (root / 'crates/an-layout/src/style.rs').read_text()
body = source[source.index('impl StyleKey'):source.index('impl Keyword')]
core = set()
for line in body.splitlines():
    m = re.match(r'\s*("(?:[^"]+)"(?:\s*\|\s*"[^"]+")*)\s*=>', line)
    if m:
        core.update(re.findall(r'"([^"]+)"', m.group(1)))

listing = (root / 'packages/platform-native/src/style-names.ts').read_text()
js = set(re.findall(r"^\s*'([^']+)'", listing, re.M))

missing = sorted(core - js)
extra = sorted(js - core)
if missing or extra:
    if missing:
        print(f"  FAIL the JS list is missing: {', '.join(missing)}")
    if extra:
        print(f"  FAIL the JS list has extra: {', '.join(extra)}")
    print("  (regenerate packages/platform-native/src/style-names.ts)")
    sys.exit(1)
print(f"  ok   the {len(core)} style names match between the core and the renderer")
PY
