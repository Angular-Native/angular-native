#!/usr/bin/env bash
# That no prop of a directive gets lost on the way.
#
# A prop travels from the directive to the core and from the core to both hosts.
# If a host does not recognise it, nothing happens: no error, no trace, and the
# control stays as it was. That is exactly what made the style bugs expensive
# —four in one day, all with the same "this does nothing" look— and here it is
# possible all over again, so it is checked the same way it is checked there.
#
# It does not prove the prop does the right thing: it proves somebody looks at
# it. That it does the right thing is what `check-controls.sh` says over the
# headless dump, and the final appearance can only be seen on the device.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

python3 - "$ROOT" <<'PY'
import pathlib, re, sys

root = pathlib.Path(sys.argv[1])
directives = (root / 'packages/primitives/src/primitives.ts').read_text()
# The whole crate and not only `host.rs`: a host may split its `set_prop` across
# several files —Apple's accessibility is in `accessibility.rs` because its six
# props are decided together— and what this checks is that the *host* looks at
# them, not that one particular file does. Searching only one would say a prop
# does not arrive when it does, which is the more expensive of the two lies.
def crate(path: str) -> str:
    return '\n'.join(f.read_text() for f in sorted((root / path).rglob('*.rs')))


ios = crate('crates/an-ios/src')
android = (root / 'shells/android/java/dev/angularnative/AnHost.java').read_text()
macos = crate('crates/an-macos/src')

# Props that are for no host: the core consumes them and there they end.
CORE_ONLY = {
    'intrinsicWidth': "the layout measures it to reserve an image's gap",
    'intrinsicHeight': "the layout measures it to reserve an image's gap",
}

# Props that only mean something where there is a pointer.
#
# They are not a hole in iOS or in Android: it is that a finger has no shape.
# Asking those two hosts to look at `cursor` would be asking them to look at
# something they cannot do, and putting it in PENDING would be saying that some
# day they will. The one that does have to look at them is the desktop host, and
# that is checked just as hard as everything else.
POINTER_ONLY = {
    'cursor': "the pointer's shape; a finger has no shape",
}

# Props that should arrive and do not arrive yet. Each with its reason in
# docs/wrapper-nativo.md. The list can only shrink.
PENDING: dict[str, str] = {}

# The common props are the keys of each directive's `push({...})`, plus those
# some of them send by hand —an image's size is written by its load listener, not
# by an input—.
common = set(re.findall(r"this\.set\('([^']+)'", directives))
for body in re.findall(r"this\.push\(\{(.*?)\n    \}\)", directives, re.S):
    common.update(re.findall(r"^      (\w+):", body, re.M))
common = sorted(common)
failures = []
pending_seen = set()
for prop in common:
    if prop in CORE_ONLY:
        continue
    if prop in POINTER_ONLY:
        if f'"{prop}"' not in macos:
            failures.append(f'  FAIL "{prop}" is not looked at by the macOS host, which is the pointer one')
        continue
    missing = [n for n, h in (('iOS', ios), ('Android', android)) if f'"{prop}"' not in h]
    if not missing:
        if prop in PENDING:
            failures.append(f'  FAIL "{prop}" already reaches both hosts: take it out of PENDING')
        continue
    if prop in PENDING:
        pending_seen.add(prop)
        continue
    failures.append(f'  FAIL "{prop}" is not looked at by {" or ".join(missing)}')

# Single-platform props: they travel with their prefix, and the prefix says who
# has to look at them. Their appearing in the other host would be a common prop
# in disguise, and then it should not carry a prefix at all.
HOSTS = {'ios': ('iOS', ios, 'Android', android), 'android': ('Android', android, 'iOS', ios)}
declared = {'ios': set(), 'android': set()}
for platform, body in re.findall(
    r"platformKeys\(\s*'[^']+',\s*'(ios|android)',\s*\[(.*?)\]\s*\)", directives, re.S
):
    declared[platform].update(re.findall(r"'([^']+)'", body))

for platform, keys in declared.items():
    own_name, own, other_name, other = HOSTS[platform]
    for key in sorted(keys):
        if f'"{platform}:{key}"' not in own:
            failures.append(f'  FAIL [{platform}] "{key}" is not looked at by the {own_name} host')
        if f'"{platform}:{key}"' in other:
            failures.append(
                f'  FAIL [{platform}] "{key}" is also looked at by {other_name}: '
                'then it is common and goes without a prefix'
            )

# The platform object has to go through `pushPlatform()`, which is what warns
# about a key nobody is going to look at. Sending it with a bare `set()` would
# work —and that is why it has to be prevented—: the key would travel and be lost
# in silence. They are counted: one `[ios]` or `[android]` input per push.
objects = len(re.findall(r"^  readonly (?:ios|android) = input<", directives, re.M))
pushes = directives.count('this.pushPlatform(')
if objects != pushes:
    failures.append(
        f'  FAIL there are {objects} platform inputs and {pushes} pushPlatform(): '
        'some unknown key would be lost without a word'
    )

for line in failures:
    print(line)
if failures:
    print('  (the inventory and the reasons are in docs/wrapper-nativo.md)')
    sys.exit(1)

print(f'  ok   the {len(common) - len(CORE_ONLY) - len(POINTER_ONLY) - len(pending_seen)} '
      'common props reach both hosts')
print('  ok   the pointer props are looked at by the desktop host: '
      + ', '.join(sorted(POINTER_ONLY)))
print(f'  ok   the {len(declared["ios"])} [ios] props are looked at by iOS alone')
print(f'  ok   the {len(declared["android"])} [android] props are looked at by Android alone')
print('  ok   every platform object goes through platform(), which warns about what it does not recognise')
if pending_seen:
    print(f'  ok   {len(pending_seen)} known pending: {", ".join(sorted(pending_seen))}')
PY
