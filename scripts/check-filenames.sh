#!/usr/bin/env bash
# That the repository can be checked out on every platform it targets.
#
# A tracked file whose name carries a character Windows forbids makes `git
# clone` fail there — not the build, the clone — and the failure names the
# path, not the reason. `crates/an-cli/src/plugins.rs:129:12` was such a file:
# a zero-byte accident, a grep location typed where a filename belonged,
# committed and unnoticed for months while Windows sat on the board as a
# platform to start.
#
# The forbidden set is Windows': < > : " | ? * , the control characters, a
# trailing dot or space, and the reserved device names. macOS and Linux take
# all of them, which is exactly why nobody here notices.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0
FILES="$(git ls-files)"

BAD="$(grep -E '[<>:"|?*]' <<<"$FILES" || true)"
if [ -z "$BAD" ]; then
  echo "  ok   no tracked path carries a character Windows forbids"
else
  echo "  FAIL these paths cannot be checked out on Windows:"
  sed 's/^/       /' <<<"$BAD"
  fail=1
fi

# A trailing dot or space is dropped silently by the Win32 API, so two paths
# that differ only by one collide on checkout.
BAD="$(grep -E '(^|/)[^/]*[ .]$' <<<"$FILES" || true)"
if [ -z "$BAD" ]; then
  echo "  ok   no tracked path ends in a dot or a space"
else
  echo "  FAIL these paths end in a dot or a space, which Windows drops:"
  sed 's/^/       /' <<<"$BAD"
  fail=1
fi

# CON, PRN, AUX, NUL, COM1-9, LPT1-9 — reserved whatever the extension.
BAD="$(grep -iE '(^|/)(con|prn|aux|nul|com[1-9]|lpt[1-9])(\.|$)' <<<"$FILES" || true)"
if [ -z "$BAD" ]; then
  echo "  ok   no tracked path is a reserved Windows device name"
else
  echo "  FAIL these paths are reserved device names on Windows:"
  sed 's/^/       /' <<<"$BAD"
  fail=1
fi

exit "$fail"
