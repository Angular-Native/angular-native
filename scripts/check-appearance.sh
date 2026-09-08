#!/usr/bin/env bash
# What the app looks like, which is a word in four places.
#
# `app.appearance` is one setting in `angular-native.json` and three different
# platform idioms underneath: `setDefaultNightMode` on Android through a
# `<meta-data>` entry, `UIUserInterfaceStyle` in the plist on the Apple
# platforms, and `NSApp.appearance` read back from that same key on the Mac.
#
# The failure this exists to stop is the ordinary one for a duplicated list:
# somebody adds a fourth value, or renames one, in the place they are working
# and not in the other three. Nothing breaks at build time — the value simply
# stops meaning anything on one platform, which is a setting that does nothing
# for a reason nobody can see. The shells already log an unknown word rather
# than guessing; this is what keeps them from ever hearing one.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0
ok() { echo "  ok   $1"; }
ko() { echo "  FAIL $1"; fail=1; }

echo "== what the app looks like"

# The vocabulary is read from the parser and not written here: a fifth
# hand-maintained copy is what this script exists to prevent.
words=$(sed -n '/fn parse(word: &str)/,/^    }$/p' crates/an-cli/src/workspace.rs \
  | grep -oE '"[a-z]+" =>' | grep -oE '"[a-z]+"' | tr -d '"' | sort)
count=$(wc -w <<<"$words" | tr -d ' ')
if [ "$count" -eq 3 ]; then
  ok "the CLI knows three appearances: $(tr '\n' ' ' <<<"$words" | sed 's/ $//')"
else
  ko "the CLI's appearance vocabulary came out as $count words, which cannot be right"
  exit 1
fi

# Android: every word the CLI can emit has an arm in the shell.
for word in $words; do
  if grep -qE "case \"$word\":" shells/android/java/dev/angularnative/MainActivity.java; then
    ok "the Android shell handles \"$word\""
  else
    ko "the Android shell has no arm for \"$word\", so it would fall through to the default"
  fi
done

# The key itself, written by the CLI and read by the shell. Two copies of a
# string, and a typo in either is a setting that silently stops arriving.
key=$(grep -oE 'APPEARANCE_KEY: &str = "[^"]+"' crates/an-cli/src/android.rs \
  | grep -oE '"[^"]+"' | tr -d '"')
if grep -qF "\"$key\"" shells/android/java/dev/angularnative/MainActivity.java; then
  ok "the CLI writes $key and the shell reads the same one"
else
  ko "the CLI writes $key and the Android shell looks for something else"
fi

# Apple: the plist key is UIKit's, and the Mac reads it back because AppKit
# does not act on it by itself.
if grep -qF 'UIUserInterfaceStyle' crates/an-cli/src/ios.rs \
  && grep -qF 'UIUserInterfaceStyle' shells/macos/Sources/AppDelegate.swift; then
  ok "the Apple platforms share one plist key, and the Mac reads it back"
else
  ko "the Apple plist key is written in one place and read in another"
fi
# Light and Dark are Apple's spelling of two of the three; `system` is the
# absence of the key, which is why only two appear.
for style in Light Dark; do
  if grep -qF "\"$style\"" crates/an-cli/src/ios.rs \
    && grep -qF "\"$style\"" shells/macos/Sources/AppDelegate.swift; then
    ok "and both agree that $style is spelled the way UIKit spells it"
  else
    ko "the CLI and the Mac shell disagree about $style"
  fi
done

# And the one platform that has no light mode does not accept a setting it
# cannot honour. What decides that is the parent of the watch's theme, not the
# comment above it: `no light mode` and `OLED` are both in the comment, so a
# parent switched to `DayNight` left the wording — and the check — untouched.
STYLES=shells/android/res/values/styles.xml
WEAR="$(grep -E '<style name="Theme\.AngularNative\.Wear"' "$STYLES" || true)"
if grep -qE 'parent="Theme\.[A-Za-z0-9]+\.Dark\.' <<<"$WEAR"; then
  ok "the watch's theme is a Dark parent, so the setting cannot turn it light"
else
  ko "Theme.AngularNative.Wear is no longer a Dark parent: ${WEAR:-the style is not there}"
fi

# And that the reason is written down where the parent is, because a fixed dark
# theme with nothing next to it reads as an oversight.
if grep -qiE 'no light mode|OLED' "$STYLES"; then
  ok "and says why: the watch's screen is OLED and black costs no power"
else
  ko "nothing says why the watch ignores the appearance"
fi

exit "$fail"
