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

# ── One spelling for a step that did not run ────────────────────────────────
#
# `check-all.sh` counts the skips at the end by grepping `^ *-- `, and prints
# "nothing was skipped: every step ran" when it finds none. Nineteen lines
# across eight scripts used to spell the same thing `skipped` or `warn`, so a
# run in which all of them fired still ended by saying every step ran, and CI —
# which promotes one literal skip line to a failure — never saw them either.
#
# So the column is fixed: a message in the `  <tag> ` position is `ok`, `FAIL`
# or `--`, and nothing else. `--` is the one that means "this did not run".
# Deeper indentation is a continuation line and is not a tag.
UNKNOWN=""
for script in scripts/check-*.sh; do
  case "${script##*/}" in
    # The one file left. It is owned elsewhere and still says `skipped`; naming
    # it is what keeps the hole visible, and the branch below deletes the name
    # from here the day it has none left.
    check-external.sh) continue ;;
  esac
  hits="$(sed 's/^[[:space:]]*#.*$//' "$script" |
    grep -nE '(echo|printf) "  [^ "]+' | grep -vE '(echo|printf) "  (ok|FAIL|--) ' || true)"
  [ -n "$hits" ] && UNKNOWN="$UNKNOWN$(sed "s|^|$script:|" <<<"$hits")"$'\n'
done
UNKNOWN="$(sed '/^$/d' <<<"$UNKNOWN")"

if [ -z "$UNKNOWN" ]; then
  echo "  ok   every check prints ok, FAIL or --, so check-all.sh's tally sees every skip"
else
  echo "  FAIL these print a tag check-all.sh does not count; a skip is \`  --   \`"
  sed 's/^/       /' <<<"$UNKNOWN"
  fail=1
fi

LEFT="$(grep -cE '(echo|printf) "  (skipped|warn)' scripts/check-external.sh || true)"
if [ "$LEFT" = 0 ]; then
  echo "  FAIL check-external.sh no longer spells a skip any other way: take it out of the"
  echo "       exception above, or the next one to appear there goes uncounted"
  fail=1
else
  echo "  ok   $LEFT lines in check-external.sh still to convert, and no other script has any"
fi

exit "$fail"
