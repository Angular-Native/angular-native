#!/usr/bin/env bash
# That every control the core asks the host to measure is actually measured by
# every host.
#
# `NodeKind::control_name()` is the whole contract: whatever it returns travels
# down to `measure_control` and comes back as the box the layout reserves. A
# host that does not recognise a name answers zero, and a control laid out 0x0
# is not an error anybody sees —it is a control that is simply not on the
# screen, in a template that looks right, with nothing in the log.
#
# That is not a hypothesis. `an-icon`, `an-segmented-control`, `an-stepper`,
# `an-search-bar`, `an-select`, `an-date-picker` and `an-navigation-bar` were
# invisible on Android for exactly this reason, and `an-navigation-bar` was on
# iOS too. Seven names in a switch nobody had lined up against the enum.
#
# So the enum is read here and lined up against each host, plus the thing that
# made the bug survive: that the fallback path says something instead of
# quietly returning zero.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== controls that get measured"

python3 - "$ROOT" <<'PY'
import pathlib, re, sys

root = pathlib.Path(sys.argv[1])
failures = []
notes = []


def section(text, start, end=None):
    """The slice of a file between two markers, so a `case` in another switch
    or a `record` in another function is not read as if it were this one's.

    The opening marker is left out of the slice: the watch's table opens with
    the same `\"\"\"` that closes it, and keeping it would end the slice on the
    first character."""
    rest = text[text.index(start) + len(start):]
    return rest[: rest.index(end)] if end else rest


# --- what the core asks for -------------------------------------------------

props = (root / 'crates/an-core/src/props.rs').read_text()
body = section(props, 'pub fn control_name(self)', '\n    }')
wanted = re.findall(r'NodeKind::(\w+) => "(\w+)"', body)
mismatched = [k for k, name in wanted if k != name]
if mismatched:
    failures.append(
        '  FAIL control_name() renames these, and every host indexes by the name: '
        + ', '.join(mismatched)
    )
CONTROLS = [name for _, name in wanted]
if not CONTROLS:
    failures.append('  FAIL control_name() could not be read out of props.rs')


def compare(host, known, exempt=()):
    missing = [c for c in CONTROLS if c not in known and c not in exempt]
    if missing:
        failures.append(f'  FAIL {host} does not measure: ' + ', '.join(missing))
        return
    stray = sorted(set(known) - set(CONTROLS))
    if stray:
        failures.append(
            f'  FAIL {host} measures what the core never asks for: ' + ', '.join(stray)
        )
        return
    tail = f' ({len(exempt)} it does not mount)' if exempt else ''
    print(f'  ok   {host} measures the {len(CONTROLS) - len(exempt)} it is asked for{tail}')


# --- Android ----------------------------------------------------------------

java = (root / 'shells/android/java/dev/angularnative/AnHost.java').read_text()
android = section(java, 'public long measureControl(', '\n    }')
compare('AnHost.measureControl', re.findall(r'case "(\w+)":', android))

# --- iOS, and tvOS and visionOS with it, which share the host ---------------

ios = (root / 'crates/an-ios/src/controls.rs').read_text()
uikit = section(ios, 'pub fn measure_controls(', '\n    sizes\n}')
compare('an-ios/controls.rs', re.findall(r'record\("(\w+)"', uikit))

# --- macOS ------------------------------------------------------------------

mac = (root / 'crates/an-macos/src/controls.rs').read_text()
appkit = section(mac, 'pub fn measure_controls(', '\n    sizes\n}')
compare(
    'an-macos/controls.rs',
    re.findall(r'record\("(\w+)"', appkit) + re.findall(r'sizes\.insert\("(\w+)"', appkit),
)

# --- watchOS ----------------------------------------------------------------
#
# The watch does not draw four of them, and says so in `unsupported()`. That is
# the only kind of gap allowed anywhere in here: one that is written down, in
# the core, with its reason.

watch_src = (root / 'crates/an-watch/src/snapshot.rs').read_text()
not_on_watch = set(
    re.findall(r'NodeKind::(\w+) =>', section(watch_src, 'pub fn unsupported(kind: NodeKind)', '\n}'))
)
runtime = (root / 'shells/watchos/Sources/AnRuntime.swift').read_text()
table = section(runtime, 'private static let controlSizes = """', '"""')
compare(
    'AnRuntime.controlSizes',
    re.findall(r'"(\w+)":\[', table),
    exempt=sorted(c for c in CONTROLS if c in not_on_watch),
)

# --- the core's own fallback ------------------------------------------------
#
# Not a host, but it is what runs with no platform behind it —the headless
# dumps every other check reads— and a name missing here measures zero there.

layout = (root / 'crates/an-layout/src/measure.rs').read_text()
fallback = section(layout, 'fn measure_control(&self, name: &str', '\n    }')
compare('an-layout default measurer', re.findall(r'"(\w+)" => \(', fallback))


# --- and that nobody answers zero without saying so -------------------------
#
# This is the half that lets the next one be caught on the device instead of
# by eye. A host that grows a `NodeKind` and forgets the case still measures
# zero at runtime; what must never happen again is that it does it silently.

for label, path, marker in [
    ('AnHost.measureControl', 'shells/android/java/dev/angularnative/AnHost.java', 'Log.e'),
    ('an-ios/measure.rs', 'crates/an-ios/src/measure.rs', 'warn_once'),
    ('an-macos/measure.rs', 'crates/an-macos/src/measure.rs', 'warn_once'),
]:
    text = (root / path).read_text()
    where = section(
        text,
        'measureControl(' if marker == 'Log.e' else 'fn measure_control(',
        '\n    }' if marker == 'Log.e' else 'fn measure_text',
    )
    if marker in where:
        notes.append(f'  ok   {label} complains before returning zero')
    else:
        failures.append(
            f'  FAIL {label} returns zero for a control it does not know and says nothing;'
            ' that silence is the reason this bug lived'
        )

for line in notes:
    print(line)
for line in failures:
    print(line)
if failures:
    sys.exit(1)
PY
