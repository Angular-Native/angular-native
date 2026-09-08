#!/usr/bin/env bash
# That no check can fail for a reason that has nothing to do with what it checks.
#
# Every script here runs under `set -euo pipefail`, and `grep -q` exits the
# moment it finds a match. The producer on the left of the pipe is then writing
# into a closed pipe, takes SIGPIPE and exits 141 — and `pipefail` hands 141 to
# the `if`. So `cmd | grep -q PATTERN` reports *not found* precisely when the
# pattern is found early and the producer still had output to write.
#
# It is worse in the `if !` form, which is how a script asks whether a tool is
# installed: the pipeline is non-zero, the `!` makes it true, and the check
# reports the thing missing and skips itself, quietly, on a machine that has it.
#
# Whether it fires depends on where the match falls and on the 64 KB pipe
# buffer, so it works on the small inputs and fails on the large ones — which
# is the failure mode this repository spends its scripts avoiding.
#
# `grep -q PATTERN <<<"$(cmd)"` is not a pipeline: the producer runs to the end
# and writes into a temporary file, and grep reads that. The `|| true` goes with
# it, because a producer that legitimately fails must not take `set -e` with it.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0
# Comments are stripped first, or this file's own explanation of the bug would
# be read as an instance of it.
BAD=""
for script in $(grep -ln pipefail scripts/*.sh); do
  hits="$(sed 's/^[[:space:]]*#.*$//' "$script" | grep -n '| *grep -q' || true)"
  [ -n "$hits" ] && BAD="$BAD$(sed "s|^|$script:|" <<<"$hits")"$'\n'
done
BAD="$(sed '/^$/d' <<<"$BAD")"

if [ -z "$BAD" ]; then
  echo "  ok   no check pipes into grep -q, so none can be defeated by SIGPIPE"
else
  echo "  FAIL these pipe into grep -q under pipefail; use grep -q … <<<\"\$(cmd || true)\""
  sed 's/^/       /' <<<"$BAD"
  fail=1
fi

# And that the replacement is really in use, so this file is not passing over an
# empty set the day somebody rewrites the checks another way.
USED="$(grep -c 'grep -q.* <<<' scripts/*.sh | awk -F: '{ total += $2 } END { print total + 0 }')"
if [ "$USED" -ge 20 ]; then
  echo "  ok   $USED checks read their input with a here-string instead"
else
  echo "  FAIL only $USED here-string greps found; this check is looking at the wrong thing"
  fail=1
fi

exit "$fail"
