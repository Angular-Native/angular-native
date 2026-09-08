#!/usr/bin/env bash
# That the Java shell compiles.
#
# `cargo test` and the headless dumps touch no Java: the Android host is 2,600
# lines that until now were only compiled when the APK was built by hand, and a
# whole batch of props could sit there with a type error without anything saying
# so. Building the APK without installing it costs half a minute and compiles
# everything: `aapt2` links the resources, `javac` compiles the shell against
# Material, and `d8` dexes it.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== Android Java shell"
# The output goes to a file and not to /dev/null: with `set -e` and `pipefail`, a
# compilation failure inside a `$(...)` kills the script without printing
# anything and the checker is left silent, which is exactly what must not happen.
LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT
if ! cargo an android examples/controls --no-launch >"$LOG" 2>&1; then
  echo "  FAIL the APK never got built"
  tail -30 "$LOG"
  exit 1
fi
APK="$(tail -1 "$LOG")"
if [ ! -f "$APK" ]; then
  echo "  FAIL the APK never got built"
  tail -30 "$LOG"
  exit 1
fi
echo "  ok   javac compiles AnHost and friends against Material 3"

# The viewport has to follow the window, and only the Java says whether it does.
# `check-rotation-device.sh` proves it on a phone, but it needs one plugged in;
# this is the part that can be asserted anywhere, and it is the part that was
# missing for as long as the bug lived: nothing ever called `setViewport`, so a
# rotation left the app laid out for the width it started with.
ACTIVITY=shells/android/java/dev/angularnative/MainActivity.java
if ! grep -q "addOnLayoutChangeListener" "$ACTIVITY"; then
  echo "  FAIL MainActivity does not watch the container's size"
  exit 1
fi
if ! grep -q "setViewport" "$ACTIVITY"; then
  echo "  FAIL MainActivity never tells the engine the viewport changed"
  exit 1
fi
echo "  ok   the viewport follows the container, so a rotation relays out"

# `flush` belongs to the mount side and to nobody else.
#
# The activity used to call it again after `nativeFrame` returned, so on every
# productive frame the container was asked for a layout twice. It cost little
# —the dialog reconcilers clear their dirty lists, so the second pass returned
# at the first line— but the activity cannot tell when the mount side already
# flushed, and it never flushed the frames mounted outside the callback. There
# is no device here to count frames on, so the caller is counted instead.
# The line comments go first: the activity says in one why it does not call it.
if sed 's://.*::' "$ACTIVITY" | grep -q "host\.flush()"; then
  echo "  FAIL MainActivity calls host.flush(); the Rust mount side already does, once per frame"
  exit 1
fi
if ! grep -q '"flush", "()V"' crates/an-android/src/host.rs; then
  echo "  FAIL the JNI host no longer calls flush, so nothing closes the frame"
  exit 1
fi
echo "  ok   flush has one caller, the mount side"

# That what the host draws on the text, it also measures.
#
# `call_java_long` clears the exception and answers `None` when Rust's
# descriptor does not name a real Java method, and the measurer then falls back
# to a zero width. So a signature that drifts does not crash and does not log:
# text just measures zero, on the device, later. The descriptor is lined up
# against the declaration here, where it costs nothing, and the two font props
# that were drawn without ever being measured are required to appear in both.
python3 - "$ROOT" <<'PY'
import pathlib, re, sys

root = pathlib.Path(sys.argv[1])
rust = (root / 'crates/an-android/src/measure.rs').read_text()
java = (root / 'shells/android/java/dev/angularnative/AnHost.java').read_text()
failures = []

descriptor = re.search(r'"measureText",\s*\n\s*"\(([^)]*)\)J"', rust)
declaration = re.search(r'public long measureText\(([^)]*)\)', java)
if not descriptor or not declaration:
    failures.append('  FAIL measureText could not be read out of one of the two sides')
else:
    JVM = {'F': 'float', 'I': 'int', 'Z': 'boolean', 'Ljava/lang/String;': 'String'}
    wanted, rest = [], descriptor.group(1)
    while rest:
        token = next((t for t in JVM if rest.startswith(t)), None)
        if token is None:
            failures.append(f'  FAIL the JNI descriptor uses a type this check cannot read: {rest}')
            break
        wanted.append(JVM[token])
        rest = rest[len(token):]
    got = [p.split()[-2] for p in declaration.group(1).split(',')]
    if wanted != got:
        failures.append(
            '  FAIL measureText: Rust calls (' + ', '.join(wanted)
            + ') and AnHost declares (' + ', '.join(got) + ')'
        )
    else:
        print(f'  ok   measureText crosses JNI with the {len(got)} arguments AnHost declares')

# Drawn and measured are one pair; the second half is what was missing.
body = java[java.index('public long measureText('):]
body = body[: body.index('\n    }')]
for prop, drawn, measured in [
    ('letterSpacing', 'setLetterSpacing', 'setLetterSpacing'),
    ('lineHeight', 'setLineSpacing', 'setLineSpacing'),
]:
    if measured not in body:
        failures.append(
            f'  FAIL {prop} is applied to the view by {drawn} and never to the measurement;'
            ' the layout would reserve a box the text does not fit'
        )
if not failures:
    print('  ok   letter spacing and line height reach the StaticLayout, not only the TextView')

for line in failures:
    print(line)
if failures:
    sys.exit(1)
PY
