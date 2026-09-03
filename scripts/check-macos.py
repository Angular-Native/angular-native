#!/usr/bin/env python3
"""The parts of the macOS host that can be read without compiling anything.

It is kept apart from the `.sh` for the same reason as on the watch: here lists
that live in different files are compared, and the shell is no place for that.
The `.sh` keeps what really is a process —compiling, building the `.app`,
launching it—.

The three things looked at are the three ways this host has of failing silently:

1. A primitive the core mounts and the inventory does not name. The `match` in
   `create` would not compile, but the report of what macOS paints and what it
   does not would still compile, and it would come out with a hole in it.
2. A `_ => {}` at the end of a props or styles `match`: the prop arrives, is not
   applied and nobody says so.
3. A shell that copies the development server client instead of using the one in
   `shells/shared`: two copies that drift apart the first time somebody touches
   one of them.
"""

import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1])
failures: list[str] = []
oks: list[str] = []


def read(path: str) -> str:
    file = root / path
    if not file.is_file():
        failures.append(f'  FAIL {path} is missing')
        return ''
    return file.read_text()


props = read('crates/an-core/src/props.rs')
support = read('crates/an-macos/src/support.rs')
host = read('crates/an-macos/src/host.rs')
events = read('crates/an-macos/src/events.rs')
flipped = read('crates/an-macos/src/flipped.rs')
plist = read('shells/macos/Resources/Info.plist')
directives = read('packages/primitives/src/primitives.ts')
cli = read('crates/an-cli/src/macos.rs')
root_swift = read('shells/macos/Sources/RootViewController.swift')

# 1. The inventory table against the core's enum.
#
# The enum is read from the core and not copied here: it is the same rule as in
# `check-kinds.sh`, where a hand-written list is exactly what one wants out of
# the way.
body = props[props.index('pub enum NodeKind {'):]
body = body[:body.index('\n}')]
# `RawText` is internal: it never gets mounted as a view, so it has no business
# being in an inventory of what gets painted.
from_core = {n for n in re.findall(r'^    ([A-Z]\w+),$', body, re.M)} - {'RawText'}

from_inventory = set(re.findall(r'NodeKind::(\w+),\s*Support::', support))
from_inventory |= set(re.findall(r'\(\s*NodeKind::(\w+),\s*$', support, re.M))

missing = sorted(from_core - from_inventory)
extra = sorted(from_inventory - from_core)
if missing:
    failures.append(
        '  FAIL the macOS inventory does not say what it paints these with: ' + ', '.join(missing)
    )
if extra:
    failures.append('  FAIL the inventory names primitives that do not exist: ' + ', '.join(extra))
if not missing and not extra and from_core:
    native = len(re.findall(r'Support::Native\(', support))
    assembled = len(re.findall(r'Support::Assembled\(', support))
    absent = len(re.findall(r'Support::Missing\(', support))
    elsewhere = len(re.findall(r'Support::Elsewhere\(', support))
    oks.append(
        f'  ok   the {len(from_core)} mountable primitives are in the inventory '
        f'({native} with a system control, {assembled} assembled, '
        f'{elsewhere} that macOS puts outside the tree, {absent} it does not ship)'
    )
    # A primitive declared absent has to report itself when mounted. There is
    # none today, and that is why the path that reported it was taken out of the
    # host: if somebody declares one again, it has to be written back or the node
    # would mount as an empty box and in silence, which is what this file is
    # after.
    if absent and 'HostView::Unsupported' not in host:
        failures.append(
            '  FAIL the inventory declares an absent primitive and the host no longer has the '
            'path that says so when mounting it'
        )
    elif not absent:
        oks.append('  ok   no primitive is left unpainted on this host')

# 2. The props dispatch ends up reporting, not keeping quiet.
#
# The `_ => {}` arms inside `set_prop` are another thing and they are fine: they
# dispatch by *view type* —putting `textAlign` on a switch does nothing and has
# no reason to report anything—. What there cannot be is a wildcard in the
# dispatch by *prop name*, which is where whatever somebody wrote in a template
# gets lost. That `match` has to end in two arms: the one for what is knowingly
# ignored and the one for what nobody knows about.
prop_body = host[host.index('fn set_prop('):] if 'fn set_prop(' in host else ''
# The host is a crate translated on its own branch, so the message it prints for
# an unknown prop is matched in either language.
if '_ if ignored_reason(key).is_some()' not in prop_body:
    failures.append('  FAIL set_prop does not consult IGNORED: the discarded is indistinguishable from the forgotten')
elif not re.search(r'prop desconocida|unknown prop', prop_body):
    failures.append('  FAIL set_prop swallows the props it does not know without saying so')
elif re.search(r'\n            _ => \{\s*\}\n        \}\n    \}', prop_body):
    failures.append('  FAIL set_prop ends in an empty wildcard')
else:
    oks.append('  ok   a prop macOS does not look at comes out on the error output')

# 3. What macOS cannot honour, said out loud and with a reason.
#
# `IGNORED` is the list of props this host deliberately does not apply. Without a
# written reason, the warning that comes out on screen is worth nothing. The list
# is trimmed before being read: in a file of sixteen hundred lines there are many
# two-string tuples that are not this one.
if 'const IGNORED' in host:
    listing = host[host.index('const IGNORED'):]
    listing = listing[:listing.index('\n];')]
    # Each entry starts at `("name"`; the reason is everything up to the next
    # one. It is cut this way, and not with a regular expression over the whole
    # string, because in Rust a long literal is split across several lines with
    # `\` and no reasonable expression puts it back together.
    chunks = re.split(r'\n    \(', '\n' + listing)
    ignored = []
    for chunk in chunks:
        m = re.match(r'\s*"(\w+)",(.*)', chunk, re.S)
        if m:
            ignored.append((m.group(1), m.group(2)))
    without_reason = [name for name, reason in ignored if len(re.findall(r'[a-zA-Z]', reason)) < 8]
    if not ignored:
        failures.append('  FAIL the IGNORED list of host.rs could not be read')
    elif without_reason:
        failures.append('  FAIL these props are ignored without saying why: ' + ', '.join(without_reason))
    else:
        oks.append(f'  ok   the {len(ignored)} props AppKit does not cover come out with their reason')

# 4. The shell uses the shared development client, not a copy.
if 'shells/shared' not in cli:
    failures.append('  FAIL the macOS build does not compile shells/shared: was the DevClient copied?')
elif any((root / 'shells/macos/Sources').glob('DevClient*.swift')):
    failures.append('  FAIL there is a DevClient inside shells/macos: the good one lives in shells/shared')
else:
    oks.append('  ok   the shell shares the development client from shells/shared')

# 5. The hot reload does not unmount what is still standing.
#
# The bug was already made once —a hot reload threw the views away and left the
# window black—, and the only defence is that the `clear()` sits behind the
# condition.
ffi = read('crates/an-macos/src/ffi.rs')
if not re.search(r'if !reply\.hot \{\s*\n\s*rt\.mount\.clear\(\);', ffi):
    failures.append(
        '  FAIL an_runtime_reload unmounts without checking whether the reload was a hot one'
    )
else:
    oks.append('  ok   the hot reload keeps the mounted views')

# 6. The viewport can move live, which is what tells a window from a phone
#    screen.
if 'an_runtime_set_viewport' not in root_swift or 'viewDidLayout' not in root_swift:
    failures.append('  FAIL the shell does not tell the core the window changed size')
else:
    oks.append('  ok   resizing the window redoes the layout')

# 7. The frameworks the host uses are named by whoever links.
#
# It is the most expensive silent failure of this host and it has already claimed
# a piece: a Rust `staticlib` does not drag its native dependencies along, so the
# crate's `#[link(kind = "framework")]` never reaches the linker. Without the
# `-framework` in the CLI's `swiftc`, the `.app` builds, is signed, starts and
# blows up on mounting the first view of that class.
declared = set()
for file in sorted((root / 'crates/an-macos/src').glob('*.rs')):
    declared.update(
        re.findall(r'#\[link\(name = "(\w+)", kind = "framework"\)\]', file.read_text())
    )
if not declared:
    failures.append('  FAIL not one framework #[link] could be read from an-macos')
else:
    unlinked = sorted(f for f in declared if f'"{f}"' not in cli)
    if unlinked:
        failures.append(
            '  FAIL the host uses these frameworks and the .app link does not name them: '
            + ', '.join(unlinked)
        )
    else:
        oks.append(
            f'  ok   the {len(declared)} frameworks of the host are named by the .app link'
        )

# 8. The navigation bar goes to the window's title bar, not to a view.
#
# It is this host's decision about `<an-navigation-bar>`, and it has two halves
# that can break separately: that the `[title]` ends up in the window, and that
# the node leaves no gap where there is nothing. Without the second, the screen
# would come out with an empty strip at the top and nobody would know where it
# came from.
controls = read('crates/an-macos/src/controls.rs')
if 'Support::Elsewhere' not in support or 'NavigationBar' not in support:
    failures.append('  FAIL the inventory does not say where the navigation bar ends up')
elif 'window.setTitle' not in host:
    failures.append('  FAIL the [title] of <an-navigation-bar> does not reach the title bar')
elif not re.search(r'"NavigationBar"\.to_owned\(\), \(0\.0, 0\.0\)', controls):
    failures.append(
        '  FAIL the navigation bar does not measure zero on macOS: it would leave an empty strip '
        'under the title bar'
    )
else:
    oks.append('  ok   the navigation bar [title] ends up in the title bar and the node takes no room')

# 9. The swipe is the system's, not a `pan` with a made-up threshold.
#
# AppKit has no swipe recogniser, and the easy way out would have been to measure
# a drag and decide on our own when it counts. The real gesture exists
# —`swipeWithEvent:`, with the threshold and the finger count the system decides—
# and that is the one to handle.
if 'swipeWithEvent' not in flipped:
    failures.append('  FAIL nobody handles swipeWithEvent:, so (swipeLeft) never arrives')
elif 'msg_send![super(self), swipeWithEvent: event]' not in flipped:
    failures.append(
        '  FAIL a view that does not listen for the swipe swallows it instead of passing it to '
        'the responder chain'
    )
else:
    oks.append('  ok   the swipe is the system event and whoever does not listen passes it on')

# 10. The pointer: hover and cursor, and not one cursor drawn by hand.
if '(hover)' in directives and 'hover' not in support:
    failures.append('  FAIL the primitive declares (hover) and the macOS host does not know it')
elif 'NSTrackingArea' not in events:
    failures.append('  FAIL (hover) is not mounted on an NSTrackingArea')
else:
    cursors = re.findall(r'"([a-z-]+)" => NSCursor::(\w+)\(\)', events)
    # The vocabulary is read from the `NativeCursor` type, trimmed before being
    # looked at: a loose expression over the whole file would pick up any other
    # union of strings and would demand an `NSCursor` for values that are not
    # cursors.
    union = directives[directives.index('export type NativeCursor ='):]
    union = union[:union.index('\n\n')]
    vocabulary = set(re.findall(r"'([a-z-]+)'", union))
    missing_cursors = sorted(vocabulary - {name for name, _ in cursors})
    if missing_cursors:
        failures.append(
            '  FAIL these cursors are accepted by the primitive and macOS does not set them: '
            + ', '.join(missing_cursors)
        )
    else:
        oks.append(
            f'  ok   the {len(cursors)} pointers are system NSCursors, none drawn by hand'
        )

# 11. Showing where you are asks for permission, and the permission is declared
#     or it is not asked for.
#
# `setShowsUserLocation:` without the key in the plist does not fail: the system
# denies the permission on its own and the dot never appears, with no error and
# nothing to look at.
if 'setShowsUserLocation' in host and 'NSLocationUsageDescription' not in plist:
    failures.append(
        '  FAIL the map asks for the location and the Info.plist does not declare what for: the '
        'permission is denied on its own and the dot never appears'
    )
elif 'setShowsUserLocation' in host:
    oks.append('  ok   the map declares what it wants the location for before asking for it')

for line in oks:
    print(line)
for line in failures:
    print(line)
sys.exit(1 if failures else 0)
