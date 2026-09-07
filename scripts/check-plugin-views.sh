#!/usr/bin/env bash
# The one hole in a closed enum: the view a plugin brings.
#
# `NodeKind` is a closed enum whose byte codes are frozen in the protocol, and
# `check-kinds.sh` is what keeps the four copies of that list agreeing. `Custom`
# is the single exception to "the core knows every kind": the byte means "ask
# the host's plugin-view registry" and the name travels as the `an:view` prop.
#
# That shape puts the failure somewhere no compiler looks. A host that grew the
# kind but never learnt the prop mounts an empty box; a shell that never
# installs the factory leaves every name unresolvable; a plugin whose name is
# not registered mounts nothing. None of those is a compile error and none of
# them is visible in a screenshot of a plugin whose view happens to be black.
#
# So what is checked here is that each half exists on each host that claims the
# feature, that the one host which cannot do it says so, and — the part worth
# more than the greps — that a real `<an-custom>` survives the protocol and
# comes out of the layout with a position and a size.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0
ok() { echo "  ok   $1"; }
ko() { echo "  FAIL $1"; fail=1; }

has() {
  if grep -qE -e "$2" "$1" 2>/dev/null; then ok "$3"; else ko "$3"; fi
}

echo "== views a plugin brings"

# ── 1. Both halves, on each host that mounts one ────────────────────────────
#
# A host needs three things and the check is three things: the Rust side that
# asks, the prop arm that knows when to ask, and the shell's registry that
# answers. Two out of three is the silent case.
for host in ios macos; do
  has "crates/an-$host/src/plugin_views.rs" 'an_plugin_view_set_factory' \
    "an-$host installs a factory the shell can fill"
  has "crates/an-$host/src/host.rs" '"an:view"' \
    "an-$host reads the name off the node instead of guessing it"
  has "shells/$host/Sources/AnPluginViews.swift" 'an_plugin_view_set_factory' \
    "the $host shell hands its registry to the core"
  has "crates/an-$host/include/"*.h 'an_plugin_view_set_factory' \
    "and the $host header declares the symbol both sides use"
done

has 'shells/android/java/dev/angularnative/AnPluginViews.java' 'register' \
  'the Android shell has a registry of its own'
has 'shells/android/java/dev/angularnative/AnHost.java' 'an:view' \
  'and the Android host reads the name off the node'

# The shell has to install the registry *before* the runtime exists: the core
# builds its modules when the engine starts, and a plugin registers its views
# from `attach`, which is what installing the plugin registry calls. Installed
# after, every name would resolve to nothing on the first frame.
for shell in ios macos; do
  file="$(ls shells/$shell/Sources/RootViewController.swift 2>/dev/null || true)"
  if [ -z "$file" ]; then
    ko "the $shell shell has a root view controller to install from"
    continue
  fi
  install=$(grep -n 'AnPluginViews.install()' "$file" | head -1 | cut -d: -f1)
  runtime=$(grep -n 'an_runtime_new' "$file" | head -1 | cut -d: -f1)
  if [ -n "$install" ] && [ -n "$runtime" ] && [ "$install" -lt "$runtime" ]; then
    ok "the $shell shell installs the registry before the runtime is created"
  else
    ko "the $shell shell installs the registry before the runtime is created"
  fi
done

# ── 2. A name nobody registered is said, once ───────────────────────────────
#
# Once per name and not once per node: a list of five hundred rows with the
# same missing view is one mistake. Once per *frame* would be worse than
# useless — the prop arrives on every change.
for host in ios macos; do
  if grep -qE 'no plugin registers a view called' "crates/an-$host/src/host.rs"; then
    ok "an-$host says which name mounted nothing"
  else
    ko "an-$host says which name mounted nothing"
  fi
  if grep -qE 'warned_views|warn_once\(format!\("plugin-view' "crates/an-$host/src/host.rs"; then
    ok "and says it once per name, not once per node"
  else
    ko "and says it once per name, not once per node"
  fi
done

# ── 3. The host that cannot, and says so ────────────────────────────────────
#
# watchOS mirrors the tree into a model SwiftUI redraws rather than mounting
# views, so there is nowhere to put one. What matters is that it is an answer
# and not an omission: a `_ => {}` there would be an empty box in silence.
has 'crates/an-watch/src/snapshot.rs' 'NodeKind::Custom' \
  'watchOS gives Custom an explicit answer rather than a wildcard'
if grep -rqE 'AnPluginViews' shells/watchos/ 2>/dev/null; then
  ko 'and the watch shell does not pretend to have a registry'
else
  ok 'and the watch shell does not pretend to have a registry'
fi

# ── 4. A real one, through the whole pipeline ───────────────────────────────
#
# The greps above cannot tell a host that reads `an:view` from one that reads
# it and drops it. This part is the only one that runs the thing: `headless`
# has no Swift and no plugin, so no view is ever produced — what it proves is
# that the kind survives the protocol, reaches the tree and is given a box by
# the layout, which is everything up to the factory call.
if cargo an build examples/plugins-apple >/dev/null 2>&1; then
  TREE="$(cargo run -q -p an-bridge --example headless -- \
            build/bundle/plugins-apple/main.js 6 2>&1 || true)"
  if grep -qE '^\s*Custom#[0-9]+ \[[0-9]+,[0-9]+ [0-9]+x[0-9]+\]' <<<"$TREE"; then
    ok "an <an-custom> reaches the tree with a position and a size: \
$(grep -oE 'Custom#[0-9]+ \[[0-9]+,[0-9]+ [0-9]+x[0-9]+\]' <<<"$TREE" | head -1)"
  else
    ko 'an <an-custom> reaches the tree with a position and a size'
    echo "$TREE" | tail -20
  fi
else
  echo "  skipped  examples/plugins-apple does not build, so the tree was not read"
fi

exit "$fail"
