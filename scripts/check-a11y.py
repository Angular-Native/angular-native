#!/usr/bin/env python3
"""That the accessibility contract and the Android host say the same thing.

These are five lists written in five places — the contract in TypeScript, the
host's prop switch, the role table in Java, the translated strings and the
table in the documentation — and none of the five finds out when another one
changes. A role added to the contract and not to the Java table does not fail:
the node keeps its view's role, which is almost always "none", and that only
shows up with a screen reader running.

It is text and not process, just like `check-wearos.py`, and that is why it
lives in Python and not in the `.sh`.
"""

import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1])
failures: list[str] = []
oks: list[str] = []


def read(path: str) -> str:
    file = root / path
    if not file.is_file():
        failures.append(f'  FAIL missing {path}')
        return ''
    return file.read_text()


contract = read('packages/primitives/src/primitives.ts')
host = read('shells/android/java/dev/angularnative/AnHost.java')
a11y = read('shells/android/java/dev/angularnative/AnAccessibility.java')
strings = read('shells/android/res/values/strings.xml')
strings_es = read('shells/android/res/values-es/strings.xml')
doc = read('docs-site/src/content/docs/accessibility/android.md')
bridge = root / 'examples/a11y/src/accessibility.ts'

# The six from the contract, plus the test identifier: on Android they end up
# in the same place as the name — `contentDescription` — and that is why they
# are checked together. If either one stopped going through the node state, the
# last to arrive would win and Angular would be deciding the order.
PROPS = [
    'accessibilityLabel',
    'accessibilityHint',
    'accessibilityRole',
    'accessibilityValue',
    'accessibilityState',
    'accessible',
    'testID',
]

# 1. The six props exist in the contract and the host looks at them.
for prop in PROPS:
    if f'readonly {prop} = input<' not in contract:
        failures.append(f'  FAIL "{prop}" is no longer in NativeVisual')
    if f'case "{prop}":' not in host:
        failures.append(f'  FAIL "{prop}" is not looked at by AnHost.setProp')
if not failures:
    oks.append(f'  ok   the {len(PROPS)} accessibility props reach AnHost')

# 2. The role vocabulary, which lives in two different languages.
#
#    It is the list that drifts most easily: adding a role to the contract is
#    one line in a TypeScript union, and nothing forces anyone to touch Java.
union = re.search(r'export type NativeRole =\n((?:\s*\|\s*\'[^\']+\'\n)+)', contract)
if not union:
    failures.append('  FAIL cannot find the NativeRole union in the contract')
    roles: set[str] = set()
else:
    roles = set(re.findall(r"'([^']+)'", union.group(1)))

# The `case` labels of the `roleOf` switch, the only place roles are translated.
table = re.search(r'private static Role roleOf\(String name\) \{(.*?)\n    \}', a11y, re.S)
if not table:
    failures.append('  FAIL cannot find roleOf() in AnAccessibility')
    in_java: set[str] = set()
else:
    in_java = set(re.findall(r'case "([^"]+)":', table.group(1)))

for role in sorted(roles - in_java):
    failures.append(
        f'  FAIL the role "{role}" is in NativeRole and not in roleOf(): the node'
        ' would keep its view\'s role and nobody would say so'
    )
for role in sorted(in_java - roles):
    failures.append(f'  FAIL roleOf() translates "{role}", which is no longer in NativeRole')
if roles and roles == in_java:
    oks.append(f'  ok   the {len(roles)} roles of NativeRole are translated by the host')

# 3. The five state keys, which are read by hand out of a JSON object.
block = re.search(r'export interface NativeAccessibilityState \{(.*?)\n\}', contract, re.S)
if not block:
    failures.append('  FAIL cannot find NativeAccessibilityState in the contract')
    keys: set[str] = set()
else:
    keys = set(re.findall(r'^  (\w+)\?:', block.group(1), re.M))
reader = re.search(r'private static void readAccessibilityState\((.*?)\n    \}', host, re.S)
read_keys = set(re.findall(r'has\("(\w+)"\)', reader.group(1))) if reader else set()
for key in sorted(keys - read_keys):
    failures.append(f'  FAIL AnHost does not read accessibilityState.{key}')
for key in sorted(read_keys - keys):
    failures.append(f'  FAIL AnHost reads accessibilityState.{key}, which no longer exists')
if keys and keys == read_keys:
    oks.append(f'  ok   the {len(keys)} accessibilityState keys are read by the host')

# 4. What a reader pronounces verbatim has to be translated.
#
#    `setRoleDescription` and `setStateDescription` translate nothing: they
#    speak the text they are given. A string that exists in English and not in
#    Spanish leaves a Spanish phone saying "partially checked", and nobody
#    warns about it because the resource resolves anyway: Android falls back to
#    `values/`.
used = set(re.findall(r'R\.string\.(an_\w+)', a11y))
declared = set(re.findall(r'<string name="(an_\w+)"', strings))
translated = set(re.findall(r'<string name="(an_\w+)"', strings_es))
for name in sorted(used - declared):
    failures.append(f'  FAIL R.string.{name} is not in res/values/strings.xml')
for name in sorted(declared - used):
    failures.append(f'  FAIL res/values/strings.xml declares {name} and nobody uses it')
for name in sorted(declared - translated):
    failures.append(f'  FAIL {name} is not translated in res/values-es/strings.xml')
if used and used == declared == translated:
    oks.append(f'  ok   the {len(used)} strings that get spoken out loud are translated')

# 5. The table in the documentation names every role.
#
#    It is the only one of the five lists a person reads, and it is the one
#    that says what each role does on Android. A role that is not there is a
#    role that exists and whose translation cannot be known without opening the
#    Java.
if doc:
    missing_from_doc = [role for role in sorted(roles) if f'`{role}`' not in doc]
    for role in missing_from_doc:
        failures.append(f'  FAIL the role "{role}" is not in the documentation table')
    if not missing_from_doc:
        oks.append('  ok   the role table in the documentation names them all')

# 6. The example's bridge, which exists because `NativeVisual` does not push
#    these six inputs yet.
#
#    This check reads the other way round from the rest: while the contract
#    does not push them, the bridge is mandatory — without it none of this can
#    be seen running; the moment it does push them, the bridge is one place too
#    many and has to be deleted.
pushed = set()
for body in re.findall(r'this\.push\(\{(.*?)\n    \}\)', contract, re.S):
    pushed.update(re.findall(r'^      (\w+):', body, re.M))
missing_from_contract = [p for p in PROPS if p != 'testID' and p not in pushed]
if missing_from_contract:
    if not bridge.is_file():
        failures.append(
            '  FAIL NativeVisual declares the six props and does not push them, and the'
            ' bridge at examples/a11y/src/accessibility.ts is gone: without one of the'
            ' two, a template writes them and nothing reaches the host'
        )
    else:
        text = bridge.read_text()
        for prop in missing_from_contract:
            if f'{prop}:' not in text:
                failures.append(f'  FAIL the example bridge does not push "{prop}"')
        oks.append(
            f'  ok   {len(missing_from_contract)} props NativeVisual does not push yet are'
            ' pushed by the example bridge'
        )
elif bridge.is_file():
    failures.append(
        '  FAIL NativeVisual already pushes the six props: delete'
        ' examples/a11y/src/accessibility.ts and drop its import from the template'
    )
else:
    oks.append('  ok   NativeVisual pushes the six props on its own')

# 7. The example exercises the whole contract.
#
#    `check-a11y-device.sh` only checks what the template asks for, so anything
#    the template stops asking for stops being checked — and nothing says so.
#    That is a check that quietly gets smaller, which is worse than one that
#    fails. Here is where the example is held to covering all of it.
example = read('examples/a11y/src/app.component.ts')
if example:
    gaps: list[str] = []
    for role in sorted(roles):
        if f"[accessibilityRole]=\"'{role}'\"" not in example:
            gaps.append(f'the role "{role}"')
    for key in sorted(keys):
        if not re.search(rf'\b{key}:', example):
            gaps.append(f'accessibilityState.{key}')
    for prop in ('accessibilityLabel', 'accessibilityHint', 'accessibilityValue', 'testID'):
        if f'[{prop}]=' not in example:
            gaps.append(prop)
    for value in ('true', 'false'):
        if f'[accessible]="{value}"' not in example:
            gaps.append(f'accessible: {value}')
    if 'mixed' not in example:
        gaps.append("checked: 'mixed'")
    for gap in gaps:
        failures.append(
            f'  FAIL examples/a11y no longer exercises {gap}: the dump would stop'
            ' checking it and nothing would say so'
        )
    if not gaps:
        oks.append('  ok   examples/a11y exercises every role, every state and both values of accessible')

for line in oks:
    print(line)
for line in failures:
    print(line)
sys.exit(1 if failures else 0)
