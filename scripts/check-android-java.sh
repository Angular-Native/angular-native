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
if grep -q "host\.flush()" <<<"$(sed 's://.*::' "$ACTIVITY" || true)"; then
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
    # The weight is nine steps from API 28, and only because both sides ask
    # `typefaceFor` for the face. A measurement that went back to computing a
    # bold-or-not style of its own would size 500 as 400 while the view drew
    # medium.
    ('fontWeight', 'applyTypeface', 'typefaceFor'),
]:
    if measured not in body:
        failures.append(
            f'  FAIL {prop} is applied to the view by {drawn} and never to the measurement;'
            ' the layout would reserve a box the text does not fit'
        )
if not failures:
    print('  ok   spacing, line height and font weight reach the StaticLayout, not only the TextView')

for line in failures:
    print(line)
if failures:
    sys.exit(1)
PY

# That the nine weights are nine and not two.
#
# There is no emulator in a checker, and `Typeface` cannot be reached from a
# desktop JVM, so the mapping is run against stubs that record what it asked
# for: `Build.VERSION.SDK_INT` becomes writable and `Typeface.create` remembers
# the weight it was handed. Nothing here reimplements the mapping — the
# bytecode invoked is the one `javac` just produced for the APK, reached by
# reflection, which is why this sits after the build and not beside the tests.
CLASSES="$ROOT/build/android/classes"
SDK_ROOT="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
ANDROID_JAR="$(ls -d "$SDK_ROOT"/platforms/*/android.jar 2>/dev/null | sort | tail -1)"
VENDOR_JARS="$(find "$ROOT/vendor/android" -name '*.jar' 2>/dev/null | tr '\n' ':')"
if [ -z "$ANDROID_JAR" ] || [ ! -d "$CLASSES" ] || ! command -v javac >/dev/null; then
  echo "  skipped the weight dump: no android.jar, no javac or no compiled shell"
  exit 0
fi

WORK="$(mktemp -d)"
trap 'rm -f "$LOG"; rm -rf "$WORK"' EXIT
mkdir -p "$WORK/src/android/os" "$WORK/src/android/graphics" "$WORK/out"

cat >"$WORK/src/android/os/Build.java" <<'JAVA'
package android.os;
public class Build {
    // Not `final`: the point of the stub is that one run can be API 24 and the
    // next API 34 without two JVMs.
    public static class VERSION { public static int SDK_INT = 34; }
    public static class VERSION_CODES { public static final int R = 30; }
}
JAVA

cat >"$WORK/src/android/graphics/Typeface.java" <<'JAVA'
package android.graphics;
/** Records the weight it is created with; the platform's picks a real cut. */
public class Typeface {
    public static final int NORMAL = 0, BOLD = 1, ITALIC = 2, BOLD_ITALIC = 3;
    public static final Typeface DEFAULT = new Typeface("system", 400, false);
    public final String family; public final int weight; public final boolean italic; public final int style;
    Typeface(String f, int w, boolean i) {
        family = f; weight = w; italic = i;
        style = (w >= 600 ? BOLD : NORMAL) | (i ? ITALIC : NORMAL);
    }
    Typeface(String f, int s) {
        family = f; style = s;
        weight = (s & BOLD) != 0 ? 700 : 400; italic = (s & ITALIC) != 0;
    }
    public static Typeface create(String family, int style) { return new Typeface(family, style); }
    public static Typeface create(Typeface base, int weight, boolean italic) {
        return new Typeface(base == null ? "system" : base.family, weight, italic);
    }
    public static Typeface defaultFromStyle(int style) { return new Typeface("system", style); }
    public static Typeface createFromAsset(Object assets, String path) { return DEFAULT; }
    public int getStyle() { return style; }
    public boolean isBold() { return (style & BOLD) != 0; }
    public boolean isItalic() { return (style & ITALIC) != 0; }
    public String toString() { return family + "/" + weight + (italic ? "/italic" : ""); }
}
JAVA

cat >"$WORK/src/WeightDump.java" <<'JAVA'
import java.lang.reflect.*;
import java.util.*;

public class WeightDump {
    public static void main(String[] args) throws Exception {
        Class<?> host = Class.forName("dev.angularnative.AnHost");
        Method weightOf = host.getDeclaredMethod("weightOf", String.class);
        Method typefaceFor = host.getDeclaredMethod("typefaceFor", String.class, int.class, boolean.class);
        weightOf.setAccessible(true);
        typefaceFor.setAccessible(true);
        Field sdk = Class.forName("android.os.Build$VERSION").getField("SDK_INT");
        boolean bad = false;

        // The keywords have to land where `font_from_props` in `an-core` lands,
        // or the drawn weight and the measured one are different numbers.
        for (String[] pair : new String[][] {{"bold", "700"}, {"normal", "400"}, {"550", "550"}, {"oops", "400"}}) {
            int got = (Integer) weightOf.invoke(null, pair[0]);
            if (got != Integer.parseInt(pair[1])) {
                System.out.println("  FAIL fontWeight=" + pair[0] + " parsed as " + got + ", not " + pair[1]);
                bad = true;
            }
        }

        for (int[] api : new int[][] {{34, 9}, {24, 2}}) {
            sdk.setInt(null, api[0]);
            Set<String> faces = new LinkedHashSet<>();
            for (int w = 100; w <= 900; w += 100) {
                faces.add(typefaceFor.invoke(null, null, w, false).toString());
            }
            if (faces.size() != api[1]) {
                System.out.println("  FAIL API " + api[0] + ": 100..900 came out as " + faces.size()
                        + " weights, expected " + api[1] + " — " + faces);
                bad = true;
            }
        }
        if (bad) {
            System.exit(1);
        }
        System.out.println("  ok   fontWeight 100..900 is nine faces from API 28 and two below it");
    }
}
JAVA

CP="$WORK/out:$ANDROID_JAR:$CLASSES:$VENDOR_JARS"
if ! javac -nowarn -cp "$CP" -d "$WORK/out" \
    "$WORK/src/android/os/Build.java" "$WORK/src/android/graphics/Typeface.java" \
    "$WORK/src/WeightDump.java" >"$LOG" 2>&1; then
  echo "  FAIL the weight dump does not compile"
  tail -20 "$LOG"
  exit 1
fi
java -cp "$CP" WeightDump
