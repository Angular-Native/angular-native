# Changelog

Everything in this repository is released together, under one version, from one
tag. That is a decision rather than a convenience, and the reason is in
[What a version means](#what-a-version-means).

Dates are the tag's. Unreleased work sits at the top until it is tagged.

## Unreleased

Nothing has been published yet. `npm view @angular-native/runtime` is a 404 and
no `v*` tag exists, so every version number below `0.1.0` is a placeholder in a
`package.json` and not something anybody can install.

## What a version means

Three things here move at different speeds, and only one of them is visible on
a package page:

- **The npm packages** — `@angular-native/runtime`, `platform`, `primitives`
  and the nine `plugin-*`.
- **The `an` binary**, which compiles an app and puts the platform bundle
  together.
- **The wire protocol** between them: twelve opcodes and a `NodeKind` byte for
  every primitive, frozen because a renderer and a core that disagree about a
  byte do not fail, they mount the wrong view. `check-kinds.sh` is what keeps
  the four copies of that list agreeing inside one commit; nothing can keep
  them agreeing *across* versions except a rule.

So the rule is that **they share a version and are released together**. A
minor bump of `@angular-native/primitives` that adds a primitive is a breaking
change to every `an` older than it — the new byte reaches `kind_from_byte` and
comes back `None` — and pretending otherwise by versioning them apart would
produce a combination that installs cleanly and mounts nothing.

Given that:

- **Patch** — a fix that changes no name and no byte. A host that drew
  something wrong now draws it right; a check that passed when it should not
  have now fails.
- **Minor** — anything added: a primitive, a style, a prop, a plugin, a
  command. New byte codes only ever go on the end, so an older core meeting a
  newer bundle refuses the byte rather than guessing at it, and an older bundle
  runs unchanged on a newer core.
- **Major** — a name or a byte that already existed means something else, a
  primitive or a prop is gone, or `an` stops accepting a project layout it used
  to. Before 1.0 the minor takes this role, which is what `0.x` is for.

Two things are deliberately outside the promise until 1.0: the Rust crates,
which are not published and whose API is the seam between parts of one binary
rather than something to build against, and the Swift and Java shells, which
are compiled from source into every app.

## What is not here yet

The 228 commits before the first tag have no entries. Writing them up
retroactively would mean deciding what a user would have noticed about a
version nobody could install, which is a story rather than a changelog. It
starts at the first tag.
