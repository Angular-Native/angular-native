#!/usr/bin/env bash
# Everything that can be verified without a device.
set -euo pipefail

# A safety net: say what fell over.
#
# A sub-script can die without ever printing its own `FAIL` —it is enough for
# `set -e` to kill it inside a `$(...)` that was covering its output—, and then
# this exited with 1 and without a single line to read, which is the worst
# possible way of failing: it looks as though nothing failed. What fell over is
# said here even when nothing was said there.
trap 'echo "  FAIL exited with an error: ${BASH_COMMAND} (check-all.sh line ${LINENO})"' ERR

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== duplicated lists"
"$ROOT/scripts/check-styles.sh"
"$ROOT/scripts/check-kinds.sh"
# The site's address is a duplicated list too — one entry, written out in four
# languages — and a stale link is the one kind of drift a reader hits before we
# do. It costs nothing, so it runs first.
"$ROOT/scripts/check-docs-url.sh"

echo
"$ROOT/scripts/check-signals.sh"

echo
echo "== props that reach both hosts"
"$ROOT/scripts/check-wrapper.sh"

echo
echo "== Rust core"
cargo test --quiet 2>&1 | tail -1

"$ROOT/scripts/check-angular.sh"
"$ROOT/scripts/check-list.sh"
"$ROOT/scripts/check-clip.sh"

echo
# The fourth duplicated list, and the one that had no check: what a control
# measures when nobody gives it a size. A name missing from a host is a control
# laid out 0x0 and invisible.
"$ROOT/scripts/check-control-sizes.sh"

echo
# One word in `angular-native.json` and three platform idioms underneath.
"$ROOT/scripts/check-appearance.sh"

echo
# That the committed cargo config carries nothing belonging to one machine.
"$ROOT/scripts/check-cargo-config.sh"

echo
# That the number of commands the pages claim is the number clap has.
"$ROOT/scripts/check-cli-commands.sh"
"$ROOT/scripts/check-publish.sh"
"$ROOT/scripts/check-plugin-sources.sh"
"$ROOT/scripts/check-router.sh"
"$ROOT/scripts/check-controls.sh"
"$ROOT/scripts/check-measure.sh"
"$ROOT/scripts/check-gestures.sh"
"$ROOT/scripts/check-pickers.sh"
"$ROOT/scripts/check-web.sh"
"$ROOT/scripts/check-media.sh"
"$ROOT/scripts/check-hot.sh"
"$ROOT/scripts/check-watchos.sh"
"$ROOT/scripts/check-tvos.sh"
"$ROOT/scripts/check-visionos.sh"

echo
"$ROOT/scripts/check-macos.sh"

echo
# The development loop, end to end. It goes here and not next to `check-hot.sh`
# because it needs the `.app` the line above just built: macOS is the one
# platform where the whole round trip —serve, build, launch, save, refresh— can
# be watched without a simulator or a person.
"$ROOT/scripts/check-dev-macos.sh"

echo
"$ROOT/scripts/check-accessibility.sh"

"$ROOT/scripts/check-external.sh"

echo
"$ROOT/scripts/check-plugins.sh"

echo
# The same system on the two hosts that used to refuse it outright. It goes
# after the one above because it leans on the same example and the same build
# directory, and it ends by driving a real Mac window: a press on the copy
# button, `NSPasteboard` written and read back, the answer on screen.
"$ROOT/scripts/check-plugins-hosts.sh"

echo
# The one thing a plugin contributes that is not a method. It goes after both
# of the above because it builds the same example, and its last step reads the
# tree that build produces.
"$ROOT/scripts/check-plugin-views.sh"

echo
# What is not a plugin: the module every host is supposed to have compiled in.
"$ROOT/scripts/check-modules.sh"

echo
# And the four the framework brings on top of it, which have a second way of
# going wrong that `device` did not: a platform without the hardware. The half
# that needs a real phone or a real watch is `check-builtins-device.sh`.
"$ROOT/scripts/check-builtins.sh"

echo
"$ROOT/scripts/check-permissions.sh"

echo
"$ROOT/scripts/check-secrets.sh"

echo
"$ROOT/scripts/check-android-java.sh"

echo
"$ROOT/scripts/check-wearos.sh"

echo
# Signing and distribution. It goes after the Android checks because it builds
# two more Android artefacts and everything they need is warm by now, and it is
# written so that it passes with no Apple account and no release keystore: what
# it checks is the plumbing and the refusals, plus the one release path that
# needs nobody's permission.
"$ROOT/scripts/check-signing.sh"

echo
# Only the half that costs nothing. The other one — build the APK, install it
# and ask the system for its accessibility tree — is `check-a11y-device.sh`,
# and it needs a device.
"$ROOT/scripts/check-a11y.sh"

echo
echo "== cross-compilation"
# Android needs the NDK in the environment. It used to come from a committed
# `.cargo/config.toml` holding one machine's paths; it is worked out at build
# time now, and `an env android` is how anything that is not `an android`
# itself — this, a CI job, an editor — gets hold of it.
# An `env` prefix, not a list of exports: most of these names carry the target
# triple with its hyphens, and a shell cannot export one of those.
ANDROID_ENV_ERR="$(mktemp)"
# Only what it printed. `2>&1` would fold cargo's own build warnings into the
# variable, and the whole thing is about to be eval'd.
ANDROID_ENV="$(cargo an env android 2>"$ANDROID_ENV_ERR")" || {
  echo "  FAIL the Android cross-compilation environment could not be worked out"
  sed 's/^/       /' "$ANDROID_ENV_ERR"
  exit 1
}
for target in aarch64-apple-ios-sim aarch64-linux-android; do
  case "$target" in
    *ios*) crate=an-ios ;;
    *) crate=an-android ;;
  esac
  # The output is kept, not thrown away: a `2>/dev/null` here cannot tell a
  # crate that does not compile from a toolchain that is not installed, and
  # the two want completely different things done about them.
  LOG="$(mktemp)"
  case "$target" in
    *android*) PREFIX="$ANDROID_ENV" ;;
    *) PREFIX="" ;;
  esac
  if eval "$PREFIX cargo build --quiet -p '$crate' --target '$target'" >"$LOG" 2>&1; then
    echo "  ok   $crate for $target"
  else
    echo "  FAIL $crate for $target"
    tail -30 "$LOG" | sed 's/^/       /'
    exit 1
  fi
done
