#!/usr/bin/env bash
# That `.cargo/config.toml` carries nothing belonging to one machine.
#
# It used to carry the whole Android cross-compilation setup, and every line of
# it held an absolute path: `/Users/somebody`, an NDK version, and a host name
# — `darwin-x86_64` — that is wrong on Linux and on any machine with a
# different NDK installed. The file is committed, so all three travelled to
# everybody who cloned the repository, and none of the three was true for any
# of them. What they got was `cc` looking for an `aarch64-linux-android-clang`
# that does not exist and QuickJS never building.
#
# The settings are worked out at build time now (`Sdk::cargo_env`). This is
# what stops them coming back: a committed file is exactly where a path like
# that is easiest to add and hardest to notice, because it keeps working for
# whoever added it.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0
ok() { echo "  ok   $1"; }
ko() { echo "  FAIL $1"; fail=1; }

echo "== the committed cargo config"

CONFIG=".cargo/config.toml"
# Comments are where this file explains itself, and the explanation names the
# paths it used to hold. Only what cargo actually reads is looked at.
SETTINGS="$(grep -vE '^\s*#' "$CONFIG" | grep -vE '^\s*$' || true)"

# An absolute path under a home directory is the shape of the original bug.
HOMEY="$(grep -nE '/Users/|/home/|\$HOME' <<<"$SETTINGS" || true)"
if [ -z "$HOMEY" ]; then
  ok "no path belonging to somebody's home directory"
else
  ko "a home directory reached the committed config, so it is one machine's again"
  echo "$HOMEY" | sed 's/^/       /'
fi

# The NDK, by any of the names it goes by. Its location, its version and the
# host tag are all discovered; none of them belongs in here.
NDKISH="$(grep -niE 'ndk|BINDGEN_EXTRA_CLANG_ARGS|darwin-x86_64|linux-x86_64|_LINKER' <<<"$SETTINGS" || true)"
if [ -z "$NDKISH" ]; then
  ok "nothing about the NDK, its version or this host's name"
else
  ko "the NDK settings are back in the committed config instead of being discovered"
  echo "$NDKISH" | sed 's/^/       /'
fi

# And what is left has to be the alias, or somebody has emptied the file by
# deleting the wrong half.
if grep -qE '^an = "run -q -p an-cli --"' <<<"$SETTINGS"; then
  ok "the cargo an alias is still there"
else
  ko "the cargo an alias is gone, and every command in the README goes with it"
fi

# The other half of the same promise: the code really does work them out, so an
# empty config is not an empty config plus a build that no longer cross-compiles.
if grep -q 'fn cargo_env' crates/an-cli/src/android.rs \
  && grep -q '\.envs(sdk.cargo_env())' crates/an-cli/src/android.rs; then
  ok "and the build hands the discovered ones to cargo itself"
else
  ko "nothing hands the Android settings to cargo, so nothing would cross-compile"
fi

exit "$fail"
