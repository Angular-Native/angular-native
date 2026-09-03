#!/usr/bin/env bash
# That the tag, the primitive's name and the protocol's code still say the same
# thing.
#
# The tag is not translated with a table: `an-` is stripped off and the rest is
# joined into PascalCase. That removes one list to maintain, but leaves a chain
# of three links —the directive's selector, the renderer's `NATIVE_KINDS`, the
# prelude's `KIND`— that can be broken by any one of them without anything
# failing visibly: a name that does not match comes out of the renderer as a
# wrapper, mounts as one extra view and raises no error. Same treatment as
# `check-styles.sh` and for the same reason.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

python3 - "$ROOT" <<'PY'
import pathlib, re, sys

root = pathlib.Path(sys.argv[1])

# The two names the core cannot change. `Picker` and `TextEditor` were chosen
# when the tag could not be called `Select` or `TextArea` because Angular does
# not self-close anything named like an HTML element; with a prefix the tag got
# its name back, but the Rust enum and the three hosts still carry the old one,
# and renaming them would mean changing the protocol.
NATURAL = {'Select': 'Picker', 'Textarea': 'TextEditor'}


def from_tag(tag):
    """The same rule as the renderer: strip `an-` and join into PascalCase."""
    return re.sub(r'(^|-)([a-z])', lambda m: m.group(2).upper(), tag[len('an-'):])


failures = []

# 1. Each directive's selector -> primitive name.
directives = (root / 'packages/primitives/src/primitives.ts').read_text()
tags = re.findall(r"@Directive\(\{ selector: '([^']+)' \}\)", directives)
unprefixed = [t for t in tags if not t.startswith('an-')]
if unprefixed:
    failures.append(f"  FAIL these tags do not carry the an- prefix: {', '.join(unprefixed)}")
from_tags = [from_tag(t) for t in tags if t.startswith('an-')]

# 2. The renderer's list.
renderer = (root / 'packages/platform-native/src/native-node.ts').read_text()
body = renderer[renderer.index('const NATIVE_KINDS = ['):renderer.index('] as const')]
from_renderer = re.findall(r"'([^']+)'", body)

# 3. The prelude's, with its code.
prelude = (root / 'packages/runtime/runtime.js').read_text()
body = prelude[prelude.index('const KIND = {'):]
body = body[:body.index('}')]
from_prelude = {n: int(c) for n, c in re.findall(r'(\w+): (\d+)', body)}

# 4. Rust's, with its code.
protocol = (root / 'crates/an-bridge/src/protocol.rs').read_text()
body = protocol[protocol.index('pub fn kind_from_byte'):protocol.index('pub fn kind_to_byte')]
from_core = {int(c): n for c, n in re.findall(r'(\d+) => NodeKind::(\w+)', body)}

missing = sorted(set(from_tags) - set(from_renderer))
extra = sorted(set(from_renderer) - set(from_tags))
if missing:
    failures.append(f"  FAIL NATIVE_KINDS is missing: {', '.join(missing)}")
if extra:
    failures.append(f"  FAIL NATIVE_KINDS has primitives with no directive: {', '.join(extra)}")

# `RawText` is written in no template: it has neither a tag nor a directive.
missing = sorted(set(from_renderer) - set(from_prelude))
extra = sorted(set(from_prelude) - set(from_renderer) - {'RawText'})
if missing:
    failures.append(f"  FAIL the prelude does not know how to send: {', '.join(missing)}")
if extra:
    failures.append(f"  FAIL the prelude sends what the renderer does not create: {', '.join(extra)}")

for name, code in sorted(from_prelude.items(), key=lambda pair: pair[1]):
    expected = NATURAL.get(name, name)
    actual = from_core.get(code)
    if actual is None:
        failures.append(f'  FAIL "{name}" travels with code {code} and Rust does not recognise it')
    elif actual != expected:
        failures.append(
            f'  FAIL "{name}" travels with code {code}, which in Rust is "{actual}"'
        )

for line in failures:
    print(line)
if failures:
    sys.exit(1)

print(f'  ok   the {len(tags)} an-* tags give their primitive name with no table in between')
print(f'  ok   the {len(from_prelude)} prelude primitives travel with the code Rust expects')
print(f'  ok   {len(NATURAL)} natural names declared: '
      + ', '.join(f'{js} is {rust} in the core' for js, rust in sorted(NATURAL.items())))
PY
