#!/usr/bin/env bash
# That every plugin's native code still compiles, on every platform it claims.
#
# `check-plugins.sh` proves the mechanism works, end to end, with one plugin.
# This proves the *others* have not rotted. A plugin's Swift and Java are only
# ever compiled when an app that depends on it is built, so a plugin nobody has
# an example for can carry a type error indefinitely — and the failure surfaces
# months later, in someone else's project, as a build that will not link.
#
# One app depends on all of them, which is what lets three builds cover the lot
# rather than three per plugin.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

APP=examples/plugins
echo "== plugin sources"

if [ ! -d "$APP" ]; then
  echo "  FAIL $APP is missing, and it is what pulls every plugin into one build"
  exit 1
fi

# Every plugin in the repository has to be a dependency of that app, or it is
# not being compiled by anything and this check is quietly covering less than it
# says. Comparing the two lists is what keeps that honest.
missing="$(python3 - "$ROOT" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
have = set(json.loads((root / 'examples/plugins/package.json').read_text()).get('dependencies', {}))
want = {json.loads(p.read_text())['name'] for p in (root / 'packages').glob('plugin-*/package.json')}
print('\n'.join(sorted(want - have)))
PY
)"
if [ -n "$missing" ]; then
  echo "  FAIL these plugins are in packages/ and nothing compiles them:"
  echo "$missing" | sed 's/^/         /'
  echo "         add them to examples/plugins/package.json and its tsconfig"
  exit 1
fi
echo "  ok   every plugin in packages/ is pulled into $APP"

LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT

build() {
  local platform="$1" what="$2"
  if cargo an "$platform" "$APP" --no-launch >"$LOG" 2>&1; then
    echo "  ok   $what"
  else
    echo "  FAIL $what"
    grep -E 'error:|error ' "$LOG" | head -20 | sed 's/^/         /'
    tail -5 "$LOG" | sed 's/^/         /'
    return 1
  fi
}

fail=0
build ios "swiftc compiles every plugin's Apple half into the .app" || fail=1
build macos "and the Mac's, which is a different protocol for attach" || fail=1
build android "javac compiles every plugin's Android half into the APK" || fail=1

exit "$fail"
