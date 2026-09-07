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

# Two apps and not one, because a plugin with no Android half makes an app that
# cannot be built for Android at all — the build refuses rather than shipping one
# whose every call would be rejected. So the app that covers three platforms
# cannot be the app that covers the Apple-only ones.
ALL=examples/plugins
APPLE=examples/plugins-apple
echo "== plugin sources"

# Every plugin has to be in whichever of the two matches what it claims, or it is
# compiled by nothing and this check quietly covers less than it says.
missing="$(python3 - "$ROOT" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
def deps(name):
    return set(json.loads((root / 'examples' / name / 'package.json').read_text()).get('dependencies', {}))
everywhere, apple = deps('plugins'), deps('plugins-apple')
problems = []
for p in sorted((root / 'packages').glob('plugin-*/package.json')):
    manifest = json.loads(p.read_text())
    name = manifest['name']
    android = 'sources' in manifest['angularNative'].get('android', {})
    wanted = 'plugins' if android else 'plugins-apple'
    if name not in (everywhere if android else apple):
        problems.append(f"    {name} covers {'android' if android else 'Apple only'} and is not in examples/{wanted}")
    if android and name in apple:
        problems.append(f"    {name} covers Android and should be in examples/plugins, not plugins-apple")
print("\n".join(problems))
PY
)"
if [ -n "$missing" ]; then
  echo "  FAIL some plugins are compiled by nothing:"
  echo "$missing"
  exit 1
fi
echo "  ok   every plugin in packages/ is compiled by one of the two apps"

LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT

build() {
  local platform="$1" app="$2" what="$3"
  if cargo an "$platform" "$app" --no-launch >"$LOG" 2>&1; then
    echo "  ok   $what"
  else
    echo "  FAIL $what"
    grep -E 'error:|error ' "$LOG" | head -20 | sed 's/^/         /'
    tail -5 "$LOG" | sed 's/^/         /'
    return 1
  fi
}

fail=0
build ios "$ALL" "swiftc compiles every cross-platform plugin into the .app" || fail=1
build macos "$ALL" "and the Mac's, which is a different protocol for attach" || fail=1
build android "$ALL" "javac compiles every Android half into the APK" || fail=1
build ios "$APPLE" "the Apple-only plugins compile too" || fail=1

exit "$fail"
