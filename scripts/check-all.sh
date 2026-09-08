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

# A step that cannot do its job prints `--` and carries on, which is the right
# behaviour and the easy one to miss: this prints a couple of thousand lines and
# one of them is a cross-compilation that never ran. Every script's output is
# copied here so the tally at the end can name them. The pipe costs nothing —
# CI already runs this with a pipe on stdout — and `pipefail` keeps the script's
# own exit status, so `set -e` still stops on the first failure.
SKIPPED="$(mktemp)"
step() {
  # The failure is reported here and not left to the ERR trap below: a trap is
  # not inherited by a function unless `errtrace` is on, and turning that on
  # makes the trap fire inside this one and name `tee` and this line instead of
  # the script that fell over. The name and the status are what somebody needs,
  # and this is the one place that has both.
  local rc=0
  "$@" 2>&1 | tee -a "$SKIPPED" || rc=$?
  if [ "$rc" -ne 0 ]; then
    echo "  FAIL ${1##*/} exited with $rc — check-all.sh stops here"
    exit "$rc"
  fi
}

# `--no-android` leaves out the half that needs the NDK. It exists for CI, which
# has to put the two halves on two runners —the rest of this wants swiftc, the
# simulators' SDKs and a Mac; the Android half wants nothing Apple— and for
# anybody working without an SDK installed. What it skips is one script,
# `check-android.sh`, so there is no list here that can drift from the list CI
# runs.
ANDROID=1
case "${1:-}" in
  "") ;;
  --no-android) ANDROID=0 ;;
  *) echo "usage: check-all.sh [--no-android]" >&2; exit 2 ;;
esac

echo "== duplicated lists"
step "$ROOT/scripts/check-styles.sh"
step "$ROOT/scripts/check-kinds.sh"
# The site's address is a duplicated list too — one entry, written out in four
# languages — and a stale link is the one kind of drift a reader hits before we
# do. It costs nothing, so it runs first.
step "$ROOT/scripts/check-docs-url.sh"

echo
step "$ROOT/scripts/check-signals.sh"

echo
echo "== props that reach both hosts"
step "$ROOT/scripts/check-wrapper.sh"

echo
echo "== Rust core"
# The output is kept rather than piped into `tail -1`. `pipefail` means a
# failing run still fails the suite, but the one line that survives is
# `error: test failed`, which names neither the crate nor the assertion; the
# reason scrolls past into the pipe. `check-watchos.sh` learned the same thing
# the same way.
CARGO_LOG="$(mktemp)"
if cargo test --quiet >"$CARGO_LOG" 2>&1; then
  tail -1 "$CARGO_LOG"
else
  echo "  FAIL the Rust tests do not pass"
  tail -40 "$CARGO_LOG"
  exit 1
fi

step "$ROOT/scripts/check-angular.sh"
step "$ROOT/scripts/check-list.sh"
step "$ROOT/scripts/check-clip.sh"
# The two things a scroll view was getting wrong on its own account: a size it
# ignored and an axis it did not have.
step "$ROOT/scripts/check-scroll.sh"

echo
# The fourth duplicated list, and the one that had no check: what a control
# measures when nobody gives it a size. A name missing from a host is a control
# laid out 0x0 and invisible.
step "$ROOT/scripts/check-control-sizes.sh"

echo
# One word in `angular-native.json` and three platform idioms underneath.
step "$ROOT/scripts/check-appearance.sh"

echo
# That the committed cargo config carries nothing belonging to one machine.
step "$ROOT/scripts/check-cargo-config.sh"

echo
# That the number of commands the pages claim is the number clap has.
step "$ROOT/scripts/check-cli-commands.sh"
step "$ROOT/scripts/check-platform-gaps.sh"
# That a check cannot report a failure that belongs to the shell rather than to
# what it was checking.
step "$ROOT/scripts/check-script-pipes.sh"
step "$ROOT/scripts/check-publish.sh"
step "$ROOT/scripts/check-plugin-sources.sh"
step "$ROOT/scripts/check-router.sh"
step "$ROOT/scripts/check-deep-links.sh"
step "$ROOT/scripts/check-controls.sh"
step "$ROOT/scripts/check-measure.sh"
step "$ROOT/scripts/check-gestures.sh"
step "$ROOT/scripts/check-pickers.sh"
step "$ROOT/scripts/check-web.sh"
step "$ROOT/scripts/check-media.sh"
step "$ROOT/scripts/check-hot.sh"
step "$ROOT/scripts/check-watchos.sh"
step "$ROOT/scripts/check-tvos.sh"
step "$ROOT/scripts/check-visionos.sh"

echo
step "$ROOT/scripts/check-macos.sh"

echo
# The development loop, end to end. It goes here and not next to `check-hot.sh`
# because it needs the `.app` the line above just built: macOS is the one
# platform where the whole round trip —serve, build, launch, save, refresh— can
# be watched without a simulator or a person.
step "$ROOT/scripts/check-dev-macos.sh"

echo
step "$ROOT/scripts/check-accessibility.sh"

step "$ROOT/scripts/check-external.sh"

echo
step "$ROOT/scripts/check-plugins.sh"

echo
# The same system on the two hosts that used to refuse it outright. It goes
# after the one above because it leans on the same example and the same build
# directory, and it ends by driving a real Mac window: a press on the copy
# button, `NSPasteboard` written and read back, the answer on screen.
step "$ROOT/scripts/check-plugins-hosts.sh"

echo
# The one thing a plugin contributes that is not a method. It goes after both
# of the above because it builds the same example, and its last step reads the
# tree that build produces.
step "$ROOT/scripts/check-plugin-views.sh"

echo
# The other thing a plugin contributes that is not a method: the files its code
# reads by name. It goes here for the same reason as the two above —the same
# example, the same build directory— and it ends by launching a real Mac window
# to see that a PNG the app does not own is on the screen.
step "$ROOT/scripts/check-plugin-resources.sh"

echo
# What is not a plugin: the module every host is supposed to have compiled in.
step "$ROOT/scripts/check-modules.sh"

echo
# And the four the framework brings on top of it, which have a second way of
# going wrong that `device` did not: a platform without the hardware. The half
# that needs a real phone or a real watch is `check-builtins-device.sh`.
step "$ROOT/scripts/check-builtins.sh"

echo
step "$ROOT/scripts/check-permissions.sh"

echo
step "$ROOT/scripts/check-secrets.sh"

echo
# The Java shell, Wear OS and the two Android cross-compilations, all in one
# script so that the CI job that owns them can name it. Skipping it is what
# `--no-android` is for, and a skip says so rather than passing quietly.
if [ "$ANDROID" = 1 ]; then
  step "$ROOT/scripts/check-android.sh"
else
  echo "== Android"
  step echo "  --   skipped: --no-android. scripts/check-android.sh is the half that was left out."
fi

echo
# Signing and distribution. It goes after the Android checks because it builds
# two more Android artefacts and everything they need is warm by now, and it is
# written so that it passes with no Apple account and no release keystore: what
# it checks is the plumbing and the refusals, plus the one release path that
# needs nobody's permission.
step "$ROOT/scripts/check-signing.sh"

echo
# Only the half that costs nothing. The other one — build the APK, install it
# and ask the system for its accessibility tree — is `check-a11y-device.sh`,
# and it needs a device.
step "$ROOT/scripts/check-a11y.sh"

echo
echo "== cross-compilation"
# Apple only. The two Android triples are cross-compiled by
# `check-android.sh`, next to the rest of the Android half, because they need
# the NDK environment and nothing else here does.
LOG="$(mktemp)"
# The output is kept, not thrown away: a `2>/dev/null` here cannot tell a crate
# that does not compile from a toolchain that is not installed, and the two want
# completely different things done about them.
if cargo build --quiet -p an-ios --target aarch64-apple-ios-sim >"$LOG" 2>&1; then
  echo "  ok   an-ios for aarch64-apple-ios-sim"
else
  echo "  FAIL an-ios for aarch64-apple-ios-sim"
  tail -30 "$LOG" | sed 's/^/       /'
  exit 1
fi

echo
echo "== skipped"
# The count, and the line each one printed. A `--` in the middle of the run is
# invisible; a `--` gathered at the end next to a number is not. The reason and
# the command that fixes it are printed where they happen, which is why this
# repeats the line rather than inventing a shorter one.
#
# It is a report and not a gate: `--no-android`, a machine with no simulator
# runtime and a keychain with no signing identity all skip legitimately, and a
# suite that failed on any of them would stop being runnable on a laptop. The
# place a skip is a bug is CI, and CI is where it is turned into one — see
# `.github/workflows/ci.yml`.
SKIPS="$(grep -cE '^ *-- ' "$SKIPPED" || true)"
if [ "$SKIPS" = 0 ]; then
  echo "  ok   nothing was skipped: every step ran"
else
  grep -E '^ *-- ' "$SKIPPED" | sed 's/^ *-- */       /'
  [ "$SKIPS" = 1 ] && word=step || word=steps
  echo "  --   $SKIPS $word did not run; the reason is printed where each one happened."
fi
