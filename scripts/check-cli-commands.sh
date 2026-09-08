#!/usr/bin/env bash
# That the number of commands written in prose is the number there are.
#
# "There are twelve commands" is the first sentence of the CLI reference and it
# is a hand-written number in three places — the English page, the Spanish one
# and the README's crate table. It went stale the moment a thirteenth was
# added, which is the ordinary fate of a count nobody derives.
#
# Nothing breaks when it drifts, which is exactly why it drifts: the page still
# renders, the commands still work, and the only person who finds out is the
# one who counts the table and gets a different answer.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0
ok() { echo "  ok   $1"; }
ko() { echo "  FAIL $1"; fail=1; }

echo "== the CLI's commands"

# Read from clap, which is the only thing that actually decides. `help` is
# clap's own and is not one of ours.
#
# The `|| true` is what keeps the branch below reachable. Under `set -e` and
# `pipefail` a binary that does not build takes the whole script with it at
# this line: no output, status 1, and a compilation error read as an invariant
# broken. The branch says which of the two it was.
COMMANDS="$(cargo an --help 2>/dev/null \
  | sed -n '/^Commands:/,/^$/p' \
  | grep -oE '^  [a-z]+' | tr -d ' ' | grep -v '^help$' | sort || true)"
COUNT="$(wc -w <<<"$COMMANDS" | tr -d ' ')"
if [ "$COUNT" -lt 5 ]; then
  ko "the command list could not be read from clap (got $COUNT): does \`cargo an --help\` build?"
  exit 1
fi
ok "the binary has $COUNT commands: $(tr '\n' ' ' <<<"$COMMANDS" | sed 's/ $//')"

# The words the two pages use, so the number can be compared without a table of
# numerals in here.
number_word() {
  case "$1" in
    10) echo "ten|diez" ;; 11) echo "eleven|once" ;; 12) echo "twelve|doce" ;;
    13) echo "thirteen|trece" ;; 14) echo "fourteen|catorce" ;;
    15) echo "fifteen|quince" ;; *) echo "" ;;
  esac
}
WORDS="$(number_word "$COUNT")"
if [ -z "$WORDS" ]; then
  ko "there is no word for $COUNT in this script; add it and the pages will be checked again"
  exit 1
fi
EN="${WORDS%%|*}"
ES="${WORDS##*|}"

# Every place the file counts them, not the first one that agrees. A file that
# says the number twice is a file where one of the two can go stale on its own,
# and a `grep -q` over the whole of it is satisfied by whichever is still right.
NUMERALS='(ten|eleven|twelve|thirteen|fourteen|fifteen|diez|once|doce|trece|catorce|quince)'
for file in docs-site/src/content/docs/reference/cli.md:"$EN" \
            docs-site/src/content/docs/es/reference/cli.md:"$ES" \
            README.md:"$EN"; do
  path="${file%%:*}"
  word="${file##*:}"
  CLAIMS="$(grep -inE "\\b$NUMERALS (commands?|comandos)\\b" "$path" || true)"
  WRONG="$(grep -ivE "\\b$word (commands?|comandos)\\b" <<<"$CLAIMS" | sed '/^$/d' || true)"
  if [ -z "$CLAIMS" ]; then
    ko "$path never counts the commands, and there are $COUNT"
  elif [ -n "$WRONG" ]; then
    ko "$path counts the commands as something other than $word, and there are $COUNT"
    sed 's/^/       /' <<<"$WRONG"
  else
    ok "$path says $word, in all $(grep -c . <<<"$CLAIMS") of the places it counts them"
  fi
done

# And every command has a row in the table, which is the other half: a count
# that matches while a command is undocumented is a count that means nothing.
for command in $COMMANDS; do
  if grep -qE "\`an $command( |\`)" docs-site/src/content/docs/reference/cli.md; then
    ok "\`an $command\` is in the reference"
  else
    ko "\`an $command\` is a command nobody documented"
  fi
done

exit "$fail"
