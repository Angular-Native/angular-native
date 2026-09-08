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
watch = crate('crates/an-watch/src')

# Four hosts and not two. `an-ios` covers tvOS and visionOS and `an-android`
# covers Wear OS, so those four are every host there is; checking iOS and
# Android alone let a prop reach the phones and stop there, which is the same
# silence this file exists to break.

# The primitives the watch does not mount at all. A prop that only ever travels
# on one of them is not a hole in that host: there is nothing there to set it
# on. Read from the same function `check-platform-gaps.sh` reads, so the two
# cannot disagree about what the watch refuses.
NATURAL = {'Picker': 'an-select', 'TextEditor': 'an-textarea'}


def tag_of(kind: str) -> str:
    if kind in NATURAL:
        return NATURAL[kind]
    return 'an-' + re.sub(r'(?<=[a-z0-9])(?=[A-Z])', '-', kind).lower()


snapshot = (root / 'crates/an-watch/src/snapshot.rs').read_text()
unsupported = re.search(r'pub fn unsupported\(kind: NodeKind\).*?\n\}', snapshot, re.S)
WATCH_REFUSES = (
    {tag_of(k) for k in re.findall(r'NodeKind::(\w+) =>', unsupported.group(0))}
    if unsupported
    else set()
)

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
# the site's "Props and the native wrapper" guide. The list can only shrink.
PENDING: dict[str, str] = {}

# The same, for the watch alone, on primitives the watch does mount.
#
# `an-watch` has no view hierarchy: it mirrors the tree into a model SwiftUI
# redraws, and a prop only arrives if `snapshot.rs` copies it into that model
# and the shell's SwiftUI reads it back. These thirteen are not copied. They are
# not refusals —nothing says they cannot be done— and they are not silent any
# more: what each of them means on a watch is on the guide page next to this
# list, and every entry taken out of here is one closed.
WATCH_PENDING = {
    'autoCapitalize': 'a TextField modifier on watchOS as everywhere else',
    'autoCorrect': 'a TextField modifier on watchOS as everywhere else',
    'bounces': 'ScrollView bounce is settable on watchOS',
    'icon': "the button's SF Symbol; watchOS has the same catalogue",
    'iconPosition': 'goes with the icon above',
    'lineHeight': 'measure.rs already computes one for the layout, and the model does not carry it',
    'maximumTrackColor': 'the slider is drawn by SwiftUI here, and its track takes a tint',
    'minimumTrackColor': 'the slider is drawn by SwiftUI here, and its track takes a tint',
    'placeholderColor': 'the placeholder itself arrives; only its colour does not',
    'refreshing': '.refreshable exists on watchOS and the model carries no flag for it',
    'returnKeyType': 'submitLabel exists on watchOS',
    'thumbColor': 'the switch and the slider both take a tint',
    'variant': "the button's shape; the model carries a title and no style",
}

# The common props are the keys of each directive's `push({...})`, plus those
# some of them send by hand —an image's size is written by its load listener, not
# by an input—.


def props_of(text: str) -> set[str]:
    found = set(re.findall(r"this\.set\('([^']+)'", text))
    for chunk in re.findall(r"this\.push\(\{(.*?)\n    \}\)", text, re.S):
        found.update(re.findall(r"^      (\w+):", chunk, re.M))
    return found


common = props_of(directives)

# Which primitive carries each prop. What comes before the first `@Directive`
# is the shared base every primitive extends, so a prop declared there has no
# owner and belongs to all of them.
blocks = re.split(r"@Directive\(\{ selector: '(an-[a-z0-9-]+)' \}\)", directives)
owners: dict[str, set[str]] = {}
for index in range(1, len(blocks), 2):
    for prop in props_of(blocks[index + 1]):
        owners.setdefault(prop, set()).add(blocks[index])

HOSTS = (('iOS', ios), ('Android', android), ('macOS', macos), ('watchOS', watch))

common = sorted(common)
failures = []
pending_seen = set()
unmounted = set()
for prop in common:
    if prop in CORE_ONLY:
        continue
    if prop in POINTER_ONLY:
        if f'"{prop}"' not in macos:
            failures.append(f'  FAIL "{prop}" is not looked at by the macOS host, which is the pointer one')
        continue
    missing = []
    for name, host in HOSTS:
        if f'"{prop}"' in host:
            continue
        if name == 'watchOS' and owners.get(prop) and owners[prop] <= WATCH_REFUSES:
            # Every primitive that carries it is one the watch does not mount.
            unmounted.add(prop)
            continue
        missing.append(name)
    if not missing:
        if prop in PENDING:
            failures.append(f'  FAIL "{prop}" already reaches every host: take it out of PENDING')
        if prop in WATCH_PENDING:
            failures.append(f'  FAIL "{prop}" already reaches the watch: take it out of WATCH_PENDING')
        continue
    if prop in PENDING:
        pending_seen.add(prop)
        continue
    if missing == ['watchOS'] and prop in WATCH_PENDING:
        pending_seen.add(prop)
        continue
    failures.append(f'  FAIL "{prop}" is not looked at by {" or ".join(missing)}')

# A name in the list that is no longer a prop is a line nobody is reading.
for stale in sorted(set(WATCH_PENDING) - set(common)):
    failures.append(f'  FAIL "{stale}" is in WATCH_PENDING and is not a prop any more')

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
    print('  (the inventory and the reasons are in docs-site, under guide/native-wrapper)')
    sys.exit(1)

print(f'  ok   the {len(common) - len(CORE_ONLY) - len(POINTER_ONLY) - len(pending_seen)} '
      'common props reach all four hosts: iOS, Android, macOS and watchOS')
if unmounted:
    print(f'  ok   and {len(unmounted)} more everywhere but the watch, which mounts no primitive '
          'that carries them: ' + ', '.join(sorted(unmounted)))
print('  ok   the pointer props are looked at by the desktop host: '
      + ', '.join(sorted(POINTER_ONLY)))
print(f'  ok   the {len(declared["ios"])} [ios] props are looked at by iOS alone')
print(f'  ok   the {len(declared["android"])} [android] props are looked at by Android alone')
print('  ok   every platform object goes through platform(), which warns about what it does not recognise')
if pending_seen:
    print(f'  ok   {len(pending_seen)} still known not to reach the watch, on primitives it does '
          f'mount: {", ".join(sorted(pending_seen))}')
PY
