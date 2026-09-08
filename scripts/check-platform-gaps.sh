#!/usr/bin/env bash
# The "What is missing" list of every platform page, against the code.
#
# Each page under `docs-site/src/content/docs/platforms/` ends with a list of
# what that platform does not do, and each has a Spanish twin. Both are prose,
# both were written by hand, and both go stale the moment a hole is closed: a
# gap that has been filled goes on being claimed —which sends somebody looking
# for a workaround they no longer need— and a gap that opens is never added,
# which is worse, because the page then reads as a promise.
#
# Nothing in a Markdown file fails to compile, so the only defence is to derive
# the list from the code the way `check-kinds.sh` derives the primitive names.
# Four things here are machine-readable and are read rather than repeated:
#
#   1. What a host refuses to mount, and why. `an-ios/src/family.rs` for tvOS,
#      `an-watch/src/snapshot.rs` for the Apple watch, `AnHost.notOnTheWatch`
#      for Wear OS, `an-macos/src/support.rs` for the Mac. iOS, visionOS,
#      macOS and Android refuse nothing, and that is checked too: a page that
#      grew a row for one of them would be inventing a gap.
#   2. `IGNORED` in the macOS host: the props AppKit accepts and drops.
#   3. `unsupported_event` in the macOS host: what warns at subscribe time.
#   4. The two languages against each other. The bullets are prose and cannot
#      be compared word for word, but the code spans inside them are the same
#      in both, so a bullet added to one page and not the other shows up here.
#
# It reads sources and Markdown and nothing else: no build, no simulator, no
# network. A source it cannot find is a `skipped`, not a quiet pass.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== the gaps the platform pages claim"

python3 - "$ROOT" <<'PY'
import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1])
oks: list[str] = []
failures: list[str] = []
missing_sources: list[str] = []


def read(path: str) -> str:
    file = root / path
    if not file.is_file():
        missing_sources.append(path)
        return ''
    return file.read_text()


# The same rule the renderer uses, run backwards: the core's name is the tag
# with `an-` stripped and the rest joined in PascalCase, so the tag is the name
# split on the humps. The two exceptions are the ones the protocol froze —
# `check-kinds.sh` carries the same pair and for the same reason.
NATURAL = {'Picker': 'an-select', 'TextEditor': 'an-textarea'}


def tag_of(kind: str) -> str:
    if kind in NATURAL:
        return NATURAL[kind]
    return 'an-' + re.sub(r'(?<=[a-z0-9])(?=[A-Z])', '-', kind).lower()


def section(text: str, heading: str) -> str | None:
    found = re.search(
        r'^## ' + re.escape(heading) + r'\s*$(.*?)(?=^## |\Z)', text, re.S | re.M
    )
    return found.group(1) if found else None


def tables(text: str) -> list[list[str]]:
    """Every Markdown table, as its list of lines."""
    out, current = [], []
    for line in text.splitlines():
        if line.startswith('|'):
            current.append(line)
        elif current:
            out.append(current)
            current = []
    if current:
        out.append(current)
    return out


def first_column_tags(table: list[str]) -> list[str]:
    """The rows whose first cell is a primitive's tag and nothing else."""
    found = []
    for row in table:
        cells = [cell.strip() for cell in row.strip().strip('|').split('|')]
        if not cells:
            continue
        name = re.fullmatch(r'`(an-[a-z0-9-]+)`', cells[0])
        if name:
            found.append(name.group(1))
    return found


# ── What each host refuses to mount ─────────────────────────────────────────

family = read('crates/an-ios/src/family.rs')
snapshot = read('crates/an-watch/src/snapshot.rs')
android_host = read('shells/android/java/dev/angularnative/AnHost.java')
support = read('crates/an-macos/src/support.rs')
macos_host = read('crates/an-macos/src/host.rs')
primitives = read('packages/primitives/src/primitives.ts')

if missing_sources:
    print('  --   skipped: ' + ', '.join(missing_sources) + ' could not be read')
    sys.exit(0)

VOCABULARY = set(re.findall(r"@Directive\(\{ selector: '(an-[^']+)' \}\)", primitives))
if not VOCABULARY:
    print('  --   skipped: not one an-* tag could be read from packages/primitives')
    sys.exit(0)


def tvos_refuses() -> set[str]:
    # The `cfg(tvos)` arm of `missing_kind`, which is the tvOS SDK's
    # availability annotation brought into Rust: the compiler cannot see it,
    # so this list is the only place it exists.
    # Inside `missing_kind`, not over the whole file: `unsupported_event` is
    # cfg'd per family as well, so a bare search finds whichever
    # `#[cfg(target_os = "tvos")]` comes first and can land in the wrong one.
    body = re.search(r'^pub fn missing_kind.*?^\}', family, re.S | re.M)
    arm = re.search(
        r'#\[cfg\(target_os = "tvos"\)\](.*?)#\[cfg\(not\(target_os = "tvos"\)\)\]',
        body.group(0) if body else '',
        re.S,
    )
    if not arm:
        return set()
    return {tag_of(k) for k in re.findall(r'NodeKind::(\w+) =>', arm.group(1))}


def watchos_refuses() -> set[str]:
    body = re.search(r'pub fn unsupported\(kind: NodeKind\).*?\n\}', snapshot, re.S)
    if not body:
        return set()
    return {tag_of(k) for k in re.findall(r'NodeKind::(\w+) =>', body.group(0))}


def wearos_refuses() -> set[str]:
    # Two Java methods: one holds the reason, the other the tag it is reported
    # under. `check-wearos.py` already checks the two agree, so either would
    # do; the tag one is read because it is the tag the page has to name.
    reasons = re.search(
        r'private static String notOnTheWatch\(int kind\) \{.*?\n    \}', android_host, re.S
    )
    names = re.search(
        r'private static String kindName\(int kind\) \{.*?\n    \}', android_host, re.S
    )
    if not reasons or not names:
        return set()
    table = dict(re.findall(r'case (KIND_\w+):\s*\n\s*return "(an-[^"]+)";', names.group(0)))
    return {table[k] for k in re.findall(r'case (KIND_\w+):', reasons.group(0)) if k in table}


def macos_refuses() -> set[str]:
    # `Support::Missing` is the arm that means "macOS does not ship it and it
    # is not imitated". There is none today and `check-macos.py` says so; if
    # one is ever declared, the Mac page has to name it.
    return {
        tag_of(k)
        for k in re.findall(r'NodeKind::(\w+),\s*Support::Missing\(', support)
    }


REFUSED = {
    'ios': set(),
    'android': set(),
    'macos': macos_refuses(),
    'tvos': tvos_refuses(),
    # visionOS is the third branch of the same `Family` enum and `missing_kind`
    # has no arm for it: the headset ships the whole of UIKit that iOS does.
    'visionos': set(),
    'watchos': watchos_refuses(),
    'wearos': wearos_refuses(),
}

PAGES = {
    'en': ('docs-site/src/content/docs/platforms', 'What is missing'),
    'es': ('docs-site/src/content/docs/es/platforms', 'Lo que falta'),
}

# The count a heading or its opening paragraph spells out. "The nine it does
# not" is a hand-written number in front of a hand-written table, and it is the
# first thing to go stale when a row is added.
# The count a heading spells out, in the two languages the site is written in.
# It starts at three: "one" and "two" turn up in a heading meaning something
# else far too often. `once` is left out on purpose — it is eleven in Spanish
# and an adverb in English, and a checker cannot tell which one it is reading.
NUMBERS: dict[str, int] = {}
for _n, (_en, _es) in enumerate(
    [
        ('three', 'tres'),
        ('four', 'cuatro'),
        ('five', 'cinco'),
        ('six', 'seis'),
        ('seven', 'siete'),
        ('eight', 'ocho'),
        ('nine', 'nueve'),
        ('ten', 'diez'),
        ('eleven', ''),
        ('twelve', 'doce'),
        ('thirteen', 'trece'),
        ('fourteen', 'catorce'),
        ('fifteen', 'quince'),
        ('sixteen', 'dieciséis'),
        ('seventeen', 'diecisiete'),
        ('eighteen', 'dieciocho'),
        ('nineteen', 'diecinueve'),
        ('twenty', 'veinte'),
    ],
    start=3,
):
    NUMBERS[_en] = _n
    if _es:
        NUMBERS[_es] = _n

for platform, refused in sorted(REFUSED.items()):
    before = len(failures)
    for language, (folder, heading) in PAGES.items():
        path = f'{folder}/{platform}.md'
        page = root / path
        if not page.is_file():
            failures.append(f'  FAIL {path} does not exist and the {language} sidebar links to it')
            continue
        text = page.read_text()

        if section(text, heading) is None:
            failures.append(f'  FAIL {path} has no "## {heading}" section')

        # 1. Code -> page. Every primitive the host turns down has to be named.
        unnamed = sorted(t for t in refused if f'`{t}`' not in text)
        if unnamed:
            failures.append(
                f'  FAIL {path} does not name ' + ', '.join(unnamed)
                + ', which this host refuses to mount'
            )

        # 2. Page -> code. A table listing primitives that are not mounted is
        #    recognised by the fact that it names at least one of them; once
        #    recognised, it has to name exactly those. That is what catches
        #    both a row for a hole that was filled and a hole with no row.
        for table in tables(text):
            listed = first_column_tags(table)
            invented = sorted(set(listed) - VOCABULARY)
            if invented:
                failures.append(
                    f'  FAIL {path} has a table row for ' + ', '.join(invented)
                    + ', which is no primitive'
                )
            if not refused or not (set(listed) & refused):
                continue
            extra = sorted(set(listed) - refused)
            short = sorted(refused - set(listed))
            if extra:
                failures.append(
                    f'  FAIL {path} lists ' + ', '.join(extra)
                    + ' among what is not mounted, and this host mounts them'
                )
            if short:
                failures.append(
                    f'  FAIL {path} is missing a row for ' + ', '.join(short)
                    + ', which this host refuses to mount'
                )

        # 3. And the number a heading spells out over that table. "The nine it
        #    does not" is a hand-written count in front of a hand-written list,
        #    and it is the first half to go stale when a row is added. Only the
        #    heading is read, not the paragraph under it: a section's prose is
        #    full of numbers that count other things.
        for block in re.split(r'^(?=## )', text, flags=re.M):
            found = [t for t in tables(block) if first_column_tags(t)]
            if len(found) != 1 or not block.startswith('## '):
                continue
            rows = len(first_column_tags(found[0]))
            for word in re.findall(r"[A-Za-zÁÉÍÓÚáéíóú]+", block.split('\n', 1)[0]):
                value = NUMBERS.get(word.lower())
                if value is not None and value != rows:
                    failures.append(
                        f'  FAIL {path}: the heading "{block.splitlines()[0][3:]}" says {value} '
                        f'and the table under it has {rows} primitives'
                    )

    # The `ok` is only earned if nothing above went wrong for this platform: a
    # green line over a red one is how a failure gets skimmed past.
    if len(failures) != before:
        pass
    elif refused:
        oks.append(
            f'  ok   the {len(refused)} primitives {platform} does not mount are on both pages, '
            'and nothing else is: ' + ', '.join(sorted(refused))
        )
    else:
        oks.append(f'  ok   {platform} mounts every primitive, and neither page claims otherwise')

# ── The macOS props AppKit takes and drops ──────────────────────────────────
#
# `IGNORED` is the host's list and the page repeats it. Two entries in it are
# not a hole in this host and have no business on the page: `ng-version`, which
# Angular writes on its own root and no template ever asks for, and the two
# intrinsic sizes, which the *core* consumes to reserve an image's box and
# never reach any host. Both are recognised rather than listed: an entry counts
# as a gap if a directive declares it as an `input()` and the core does not
# read it.
if 'const IGNORED' in macos_host:
    listing = macos_host[macos_host.index('const IGNORED'):]
    listing = listing[: listing.index('\n];')]
    ignored = re.findall(r'\(\s*"([\w-]+)"', listing)
    inputs = set(re.findall(r'^\s*readonly (\w+) = input', primitives, re.M))
    core = read('crates/an-core/src/props.rs') + read('crates/an-core/src/tree.rs')
    expected = {p for p in ignored if p in inputs and f'"{p}"' not in core}
    if not expected:
        failures.append('  FAIL the IGNORED list of the macOS host could not be read')
    else:
        before = len(failures)
        for language, (folder, heading) in PAGES.items():
            path = f'{folder}/macos.md'
            body = section((root / path).read_text(), heading) or ''
            # The bullet that carries the list is the one that names most of
            # it; found that way rather than by its wording, which is prose and
            # is not the same sentence in the two languages. Once found, what it
            # names has to be the list exactly: a prop left out is a gap the
            # page hides, and one left in is a hole that was filled and is still
            # being claimed.
            bullets = re.split(r'^- ', body, flags=re.M)[1:]
            bullet = max(
                bullets,
                key=lambda b: len({p for p in expected if f'`{p}`' in b}),
                default='',
            )
            named = {p for p in re.findall(r'`(\w+)`', bullet) if p in inputs}
            absent = sorted(expected - named)
            stale = sorted(named - expected)
            if absent:
                failures.append(
                    f'  FAIL {path}: AppKit drops these props and the list does not say so: '
                    + ', '.join(absent)
                )
            if stale:
                failures.append(
                    f'  FAIL {path} lists these among the props that are dropped, and the macOS '
                    'host no longer ignores them: ' + ', '.join(stale)
                )
        if len(failures) == before:
            oks.append(
                f'  ok   the {len(expected)} props AppKit takes and drops are on both macOS pages'
            )

# ── The macOS events that warn at subscribe time ────────────────────────────
pairs = []
if 'pub fn unsupported_event' in support:
    body = support[support.index('pub fn unsupported_event'):]
    body = body[: body.index('\n}')]
    pairs = re.findall(r'\(NodeKind::(\w+), "(\w+)"\)', body)
if pairs:
    for language, (folder, heading) in PAGES.items():
        path = f'{folder}/macos.md'
        section_body = section((root / path).read_text(), heading) or ''
        for kind, event in pairs:
            if f'({event})' not in section_body:
                failures.append(
                    f'  FAIL {path} does not say that ({event}) on <{tag_of(kind)}> is refused'
                )
    oks.append(
        f'  ok   the {len(pairs)} events macOS refuses at subscribe time are on both macOS pages: '
        + ', '.join(f'({e}) on <{tag_of(k)}>' for k, e in pairs)
    )
else:
    failures.append('  FAIL unsupported_event could not be read from the macOS host')

# ── The events iOS and Android refuse at subscribe time ─────────────────────
#
# The same list, in the shape each language allows. `an-ios` keeps it in
# `family::unsupported_event`, split into what none of the three UIKit families
# delivers and what only one of them turns down, the way `missing_kind` is
# split; `AnHost` keeps two static methods, one for every Android and one for
# the phone, because the crown does arrive on a watch.
#
# What is read is the event names, because a name is what a page has to carry.


def rust_arms(body: str) -> set[str]:
    """The event names in the patterns of a match whose arms return `Some`."""
    found: set[str] = set()
    for arm in re.findall(r'^\s+(.+?)\s*=>\s*Some\($', body, re.M):
        found.update(re.findall(r'"(\w+)"', arm))
    return found


def rust_fn(source: str, signature: str, guard: str = '') -> str:
    prefix = re.escape(guard) + r'\s*\n' if guard else ''
    found = re.search(prefix + re.escape(signature) + r'.*?\n\}', source, re.S)
    return found.group(0) if found else ''


def java_method(source: str, signature: str) -> str:
    found = re.search(re.escape(signature) + r'.*?\n    \}', source, re.S)
    return found.group(0) if found else ''


every_family = rust_arms(rust_fn(family, 'fn on_every_family('))
android_always = set(
    re.findall(r'case "(\w+)":', java_method(android_host, 'private static String unsupportedEvent('))
)
android_phone = set(
    re.findall(
        r'case "(\w+)":', java_method(android_host, 'private static String unsupportedOnThePhone(')
    )
)

REFUSED_EVENTS = {
    'ios': every_family
    | rust_arms(rust_fn(family, 'fn on_this_family(', '#[cfg(target_os = "ios")]')),
    'tvos': every_family
    | rust_arms(rust_fn(family, 'fn on_this_family(', '#[cfg(target_os = "tvos")]')),
    'visionos': every_family
    | rust_arms(rust_fn(family, 'fn on_this_family(', '#[cfg(target_os = "visionos")]')),
    'android': android_always | android_phone,
    'wearos': android_always,
}

if not every_family:
    failures.append('  FAIL unsupported_event could not be read from crates/an-ios/src/family.rs')
if not android_always or not android_phone:
    failures.append('  FAIL the two unsupportedEvent methods could not be read from AnHost')

for platform, events in sorted(REFUSED_EVENTS.items()):
    if not events:
        continue
    before = len(failures)
    for language, (folder, heading) in PAGES.items():
        path = f'{folder}/{platform}.md'
        page = root / path
        if not page.is_file():
            continue
        body = section(page.read_text(), heading) or ''
        for event in sorted(events):
            if f'({event})' not in body:
                failures.append(
                    f'  FAIL {path} does not say that ({event}) is refused at subscribe time'
                )
    if len(failures) == before:
        count = len(events)
        oks.append(
            f'  ok   the {count} event{"" if count == 1 else "s"} {platform} refuses at '
            'subscribe time '
            + ('is' if count == 1 else 'are')
            + ' on both pages: '
            + ', '.join(f'({e})' for e in sorted(events))
        )

# ── The two languages, against each other ───────────────────────────────────
#
# The prose differs, of course. What cannot differ is what is inside the
# backticks: a prop, a tag, a class or an API is the same word in both, so the
# multiset of code spans is the one thing a translation has to preserve. It is
# how a bullet added to one page and forgotten on the other is caught, and it
# is stricter than counting the bullets, which would pass on two lists that
# happen to be the same length.
for platform in sorted(REFUSED):
    spans = {}
    for language, (folder, heading) in PAGES.items():
        page = root / f'{folder}/{platform}.md'
        if not page.is_file():
            continue
        body = section(page.read_text(), heading)
        if body is None:
            continue
        # A long literal in a bullet is wrapped, so the newline inside a span
        # is not a difference between the two pages: it is where the line ended.
        spans[language] = sorted(
            ' '.join(code.split()) for code in re.findall(r'`([^`]+)`', body)
        )
        spans[f'{language}-bullets'] = len(re.findall(r'^- ', body, re.M))
    if 'en' not in spans or 'es' not in spans:
        continue
    if spans['en-bullets'] != spans['es-bullets']:
        failures.append(
            f'  FAIL {platform}: the English list has {spans["en-bullets"]} entries and the '
            f'Spanish one {spans["es-bullets"]}'
        )
    elif spans['en'] != spans['es']:
        only_en = sorted(set(spans['en']) - set(spans['es']))
        only_es = sorted(set(spans['es']) - set(spans['en']))
        detail = []
        if only_en:
            detail.append('only in English: ' + ', '.join(only_en))
        if only_es:
            detail.append('only in Spanish: ' + ', '.join(only_es))
        failures.append(
            f'  FAIL {platform}: the two lists do not name the same things — '
            + '; '.join(detail or ['the same names in a different order'])
        )

if not failures:
    oks.append(
        f'  ok   the {len(REFUSED)} lists say the same in both languages, entry for entry'
    )

for line in oks:
    print(line)
for line in failures:
    print(line)
sys.exit(1 if failures else 0)
PY
