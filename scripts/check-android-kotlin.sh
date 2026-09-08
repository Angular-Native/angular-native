#!/usr/bin/env bash
# That a plugin written in Kotlin reaches the dex.
#
# Kotlin is the language Android is written in, so a plugin author reaches for a
# `.kt` before a `.java`. There is no Gradle here to apply a Kotlin plugin for
# us: `an` runs `kotlinc` itself, before `javac`, into the same classes
# directory, and only when a plugin actually brought a `.kt`.
#
# Three things can go wrong and none of them is loud on its own:
#
#   1. The Kotlin is left out of the build. The APK is signed, installs, and
#      dies looking for a class the registry names.
#   2. The two compilations stop seeing each other. Kotlin calling Java or the
#      generated Java registry calling Kotlin is what breaks, at `javac` time.
#   3. The compiler is not on the machine. That has to be said, naming the file,
#      rather than an APK coming out with a plugin missing from it.
#
# The plugin is a fixture under `build/` and not a package in `packages/`, so
# that nobody has to download a 55 MB compiler to build an app with a reference
# plugin in it. `check-plugin-sources.sh` covers those; this covers the language.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== Android Kotlin plugins"

SDK_ROOT="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
if [ ! -d "$SDK_ROOT/platforms" ] || [ ! -s vendor/android/build/classpath.txt ]; then
  echo "  skipped: no Android SDK or no vendored dependencies"
  exit 0
fi
if [ ! -f vendor/android/tools/kotlinc/kotlin-compiler.jar ]; then
  echo "  skipped: no Kotlin compiler; python3 scripts/fetch-android-deps.py fetches it"
  exit 0
fi

fail=0
ok() { echo "  ok   $1"; }
ko() { echo "  FAIL $1"; fail=1; }

# ── The fixture ─────────────────────────────────────────────────────────────
#
# Two levels below the root and no deeper: the example's tsconfig reaches the
# packages as `../../packages`, so a copy anywhere else compiles against
# nothing.
APP="$ROOT/build/kotlin-fixture"
PLUGIN="$APP/node_modules/@fixture/kotlin"
rm -rf "$APP"
mkdir -p "$PLUGIN/native/android/dev/angularnative/plugins"
cp -R examples/hello-angular/src "$APP/src"
# Its own output directory: sharing hello-angular's would leave the real
# example's build with this one's bundle in it.
sed 's#build/js/hello-angular#build/js/kotlin-fixture#' \
  examples/hello-angular/tsconfig.json >"$APP/tsconfig.json"
cat >"$APP/package.json" <<'JSON'
{
  "name": "@fixture/kotlin-app",
  "version": "0.0.1",
  "private": true,
  "type": "module",
  "dependencies": { "@fixture/kotlin": "0.0.1" }
}
JSON
cat >"$PLUGIN/package.json" <<'JSON'
{
  "name": "@fixture/kotlin",
  "version": "0.0.1",
  "angularNative": {
    "module": "kotlinfixture",
    "android": {
      "sources": "native/android",
      "register": "dev.angularnative.plugins.KotlinFixturePlugin"
    }
  }
}
JSON

# Kotlin calling Java: the plugin implements `AnPlugin`, which is the shell's,
# and a helper of its own, which is the plugin's. Both are compiled in this same
# build, so if `kotlinc` were handed only the `.kt` files neither would resolve.
cat >"$PLUGIN/native/android/dev/angularnative/plugins/KotlinFixturePlugin.kt" <<'KT'
package dev.angularnative.plugins

import android.app.Activity
import dev.angularnative.AnPlugin
import dev.angularnative.AnPluginCall
import org.json.JSONObject

class KotlinFixturePlugin : AnPlugin {
    private var host: Activity? = null

    override fun attach(host: Activity) {
        this.host = host
    }

    override fun call(method: String, args: JSONObject, respond: AnPluginCall) {
        when (method) {
            "greet" -> respond.resolve(KotlinFixtureNames.greeting())
            else -> respond.reject("the kotlin fixture has no method $method")
        }
    }
}
KT
cat >"$PLUGIN/native/android/dev/angularnative/plugins/KotlinFixtureNames.java" <<'JAVA'
package dev.angularnative.plugins;

/** Java the Kotlin above calls, which only resolves if both go into one build. */
public final class KotlinFixtureNames {
    public static String greeting() {
        return "hola";
    }
}
JAVA

# ── The compiler is missing ─────────────────────────────────────────────────
#
# First, because it is the cheap one: it stops before `cargo` is spawned. A path
# that is not a Kotlin distribution is the same case as no distribution at all.
if MISSING="$(AN_KOTLINC=/nonexistent/kotlinc cargo an android build/kotlin-fixture --no-launch 2>&1)"; then
  ko 'a build with no Kotlin compiler should not succeed'
else
  ok 'with no Kotlin compiler the build stops'
  if grep -q 'KotlinFixturePlugin.kt' <<<"$MISSING"; then
    ok 'and the message names the file it cannot compile'
  else
    ko 'and the message names the file it cannot compile'
    grep -A4 -i kotlin <<<"$MISSING" | head -10 | sed 's/^/         /'
  fi
  if grep -q 'fetch-android-deps.py' <<<"$MISSING"; then
    ok 'and says how to get one'
  else
    ko 'and says how to get one'
  fi
fi

# ── And the real build ──────────────────────────────────────────────────────
LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT
if cargo an android build/kotlin-fixture --no-launch >"$LOG" 2>&1; then
  APK="$(tail -1 "$LOG")"
else
  APK=""
fi
if [ -z "$APK" ] || [ ! -f "$APK" ]; then
  echo "  FAIL the APK with the Kotlin plugin never got built"
  tail -30 "$LOG" | sed 's/^/         /'
  exit 1
fi
ok 'kotlinc and javac compile one plugin between them into an APK'

if grep -q '==> kotlinc' "$LOG"; then
  ok 'and the build says so, so nobody has to guess which compiler ran'
else
  ko 'the build never announced kotlinc'
fi

# The class descriptors live in the dex string table as plain text, which is
# what makes this checkable without an emulator. `d8` is handed the classes
# directory and nothing about Kotlin, so a `.kt` that never compiled leaves the
# APK signed and this the only thing that notices.
# Unpacked to a file and not piped into grep: `grep -q` stops at the first hit
# and, under `pipefail`, unzip dying of SIGPIPE fails the whole pipeline.
DEX="$(mktemp)"
trap 'rm -f "$LOG" "$DEX"' EXIT
unzip -p "$APK" 'classes*.dex' >"$DEX"
if grep -qa 'Ldev/angularnative/plugins/KotlinFixturePlugin;' "$DEX"; then
  ok 'the Kotlin class is in the dex'
else
  ko 'the Kotlin class is not in the dex; the APK ships a plugin with no code'
fi

# The registry is Java naming a Kotlin class. It is generated on every build, so
# this is the direction that would break first if the two compilations were run
# against separate output directories.
GENERATED="build/android/gen-plugins/dev/angularnative/AnGeneratedPlugins.java"
if grep -q 'new dev.angularnative.plugins.KotlinFixturePlugin()' "$GENERATED"; then
  ok 'the generated Java registry instantiates the Kotlin plugin'
else
  ko 'the generated Java registry does not name the Kotlin plugin'
fi

# Kotlin only when asked for. The whole point of not applying a toolchain
# everywhere is that an app with no Kotlin in it never pays for one, and the
# only way that stays true is to check it.
JAVA_ONLY="$(mktemp)"
if cargo an android examples/clipboard --no-launch >"$JAVA_ONLY" 2>&1; then
  if grep -q '==> kotlinc' "$JAVA_ONLY"; then
    ko 'an app whose plugins are all Java still ran kotlinc'
  else
    ok 'an app whose plugins are all Java never runs kotlinc'
  fi
else
  ko 'the Java-only plugin app stopped building'
  tail -20 "$JAVA_ONLY" | sed 's/^/         /'
fi
rm -f "$JAVA_ONLY"

rm -rf "$APP"
exit "$fail"
