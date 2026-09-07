#!/usr/bin/env bash
# The natural size of a control, which is a duplicated list nobody was checking.
#
# A control has a size the platform decides — how tall a switch is, how wide a
# stepper — and the core cannot know it: it asks the host, by name, through
# `NodeKind::control_name()`. Thirteen names go out. Every host has to have an
# answer for each of them, and each keeps its own list because each gets the
# number a different way: iOS and macOS build the real control and ask it,
# Android builds a throwaway probe, and watchOS cannot ask anything at all —
# there is no object before SwiftUI draws one — so its numbers are written out.
#
# A name missing from one of those lists is a control laid out **0x0**. It is
# mounted, it is in the tree, it takes no room and it cannot be seen, and on
# three of the four hosts nothing is said. That is the silent failure the rest
# of this repository builds check scripts against, and this list did not have
# one: the platform pages claimed for a long time that six Android controls
# measured zero, long after all thirteen were handled, and nobody could tell
# from the outside whether that was true.
#
# watchOS is the one host allowed a gap, and only where it has earned it: it
# needs no size for a control it does not draw. That is not an exception, it is
# the same rule read against a smaller vocabulary, and `unsupported()` is where
# it says which ones those are.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0
ok() { echo "  ok   $1"; }
ko() { echo "  FAIL $1"; fail=1; }

echo "== the natural size of a control"

# The core's list is the question every host is answering, so it is read from
# the core and not written here: a fourth hand-maintained copy is the thing
# `check-kinds.sh` exists to avoid.
core=$(sed -n '/pub fn control_name/,/^    }$/p' crates/an-core/src/props.rs \
  | grep -oE '=> "[A-Za-z]+"' | grep -oE '"[A-Za-z]+"' | tr -d '"' | sort -u)
count=$(wc -w <<<"$core" | tr -d ' ')
if [ "$count" -lt 10 ]; then
  ko "the core's control list came out with $count names, which cannot be right"
  exit 1
fi

# iOS and macOS record theirs the same way, so they are read the same way. The
# `sizes.insert` is the second door: macOS uses it for the header, whose size is
# deliberately zero rather than measured.
ios=$(grep -oE '(record|sizes.insert)\("[A-Za-z]+"' crates/an-ios/src/controls.rs \
  | grep -oE '"[A-Za-z]+"' | tr -d '"' | sort -u)
macos=$(grep -oE '(record|sizes.insert)\("[A-Za-z]+"' crates/an-macos/src/controls.rs \
  | grep -oE '"[A-Za-z]+"' | tr -d '"' | sort -u)
# Android's are the arms of the probe's switch.
android=$(sed -n '/public long measureControl/,/^    }$/p' \
  shells/android/java/dev/angularnative/AnHost.java \
  | grep -oE 'case "[A-Za-z]+"' | grep -oE '"[A-Za-z]+"' | tr -d '"' | sort -u)
# The watch's are the keys of a JSON literal, because there is nothing to ask.
watch=$(sed -n '/private static let controlSizes/,/"""$/p' \
  shells/watchos/Sources/AnRuntime.swift \
  | grep -oE '"[A-Za-z]+":' | grep -oE '"[A-Za-z]+"' | tr -d '"' | sort -u)
# And what the watch does not draw at all, which is the only excuse for a gap.
undrawn=$(sed -n '/pub fn unsupported/,/^}$/p' crates/an-watch/src/snapshot.rs \
  | grep -oE 'NodeKind::[A-Za-z]+' | sed 's/NodeKind:://' | sort -u)

for host in ios macos android; do
  case "$host" in
    ios) have="$ios" ;;
    macos) have="$macos" ;;
    android) have="$android" ;;
  esac
  missing=$(comm -23 <(echo "$core") <(echo "$have") | tr '\n' ' ' | sed 's/ $//')
  if [ -z "$missing" ]; then
    ok "$host answers for all $count controls the core asks about"
  else
    ko "$host has no size for: $missing — they are mounted and laid out 0x0"
  fi
  # The other direction matters too: a name recorded that the core never asks
  # for is a measurement nobody reads, and usually a rename half done.
  extra=$(comm -13 <(echo "$core") <(echo "$have") | tr '\n' ' ' | sed 's/ $//')
  [ -z "$extra" ] || ko "$host records a size the core never asks for: $extra"
done

# watchOS: a size, or a written reason for not drawing it. Nothing else.
gaps=""
for name in $core; do
  grep -qx "$name" <<<"$watch" && continue
  grep -qx "$name" <<<"$undrawn" && continue
  gaps="$gaps $name"
done
if [ -z "$gaps" ]; then
  drawn=$(wc -w <<<"$watch" | tr -d ' ')
  ok "watchOS has a size for the $drawn controls it draws, and a reason for the rest"
else
  ko "watchOS neither sizes nor refuses:$gaps — they come out 0x0 with nothing said"
fi

# And the reverse for the watch: a size for something it refuses to draw is a
# number that can never be read, and a sign the two lists moved apart.
for name in $watch; do
  if grep -qx "$name" <<<"$undrawn"; then
    ko "watchOS carries a size for $name and also refuses to draw it"
  fi
done

exit "$fail"
