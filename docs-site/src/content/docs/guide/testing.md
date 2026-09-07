---
title: Checking it without a device
description: One command runs the lot, `headless` renders a bundle with no platform under it, and a handful of scripts guard the lists that are duplicated across three languages.
sidebar:
  order: 9
---

Most of what can go wrong here goes wrong **quietly**. A prop that no host
recognises does not raise anything — it travels, nobody reads it, and the
control stays as it was. A primitive whose name drifted between the renderer and
the protocol mounts as one extra wrapper view. Neither shows up as an error;
both show up as "this does nothing", hunted in the wrong place, an afternoon at
a time.

So the suite is not mostly unit tests. It is mostly *scripts that read two files
and demand they still agree*, plus a renderer that runs the whole pipeline with
no platform under it.

```bash
./scripts/check-all.sh
```

That is everything that can be verified without a device: the Rust tests, the
duplicated lists, the example apps, the accessibility trees, the plugins, an
Angular project from outside the repo, a real macOS `.app` that is launched and
screenshotted, and the two cross-compilations. Around thirty scripts run under
it and each prints its own `ok` lines.

A script that cannot do its job prints **`skipped`**, not a pass. That
distinction is the point: a suite that goes green because the emulator was not
running is worse than one that goes red.

## `headless`

```bash
cargo an build examples/hello-angular
cargo run -p an-bridge --example headless -- build/bundle/hello-angular/main.js 6
```

It assembles the whole pipeline except the platform: QuickJS evaluates the
bundle, the shadow tree takes the mutations, taffy resolves the layout, the diff
produces the mount operations — and the host is a recorder that writes down what
it was handed instead of creating views.

The clock is fake. `6` is a number of frames, and an optional third argument is
the milliseconds each one advances. Since JS here has no clock of its own —
timers advance with the vsync — a test can simulate ten seconds without waiting
ten seconds, and it gets the same answer every time.

It also simulates a press, a drag, a scroll and a back, then prints the resolved
tree:

```text
View#1 [0,0 393x852]
  Text#2 [16,115 361x33] "angular-native"
  View#3 [16,164 361x88]
    View#4 [0,0 116x88]
    View#5 [128,0 233x88]
-- frame 4 (t=4000ms): 1 operation
```

Every position in that dump is the number the core computed, which is why the
check scripts assert on exact frames rather than on "it rendered". It is the
fastest way to debug a template: no simulator, no build of a `.app`, about a
second.

The recorder deliberately writes down **every prop it was handed**, not just the
ones that change the geometry. Without that, a prop that arrives correctly and a
prop that never arrives look identical in a dump — neither moves anything — and
checking that `[variant]` or `[ios]` travelled would be impossible without a
device.

## The lists that must not drift

Three lists exist in more than one language because they have to, and three
scripts exist because of that.

| Script | What it compares |
|---|---|
| `check-kinds.sh` | The directive's selector → `NATIVE_KINDS` in the renderer → `KIND` in the prelude → `kind_from_byte` in Rust. Four links, one name. |
| `check-styles.sh` | The JS list of style names against the core's. |
| `check-wrapper.sh` | Every prop a directive declares against the hosts that are supposed to read it — including that an `[ios]` key appears in the iOS host and *not* in the Android one. |

They are string-matching scripts and they are unglamorous, and they are the
highest-value tests in the repository. `check-kinds.sh` in particular is what
lets the tag→name rule stay a rule instead of becoming a translation table
somebody has to remember to update.

`check-signals.sh` is in the same family, and it bans rather than compares:
no `@Input`, no `@Output`, no `EventEmitter`, no `@ViewChild`, no `@HostBinding`.
Not for taste — an `@Input() set` runs at the moment Angular writes the input,
so the order of writes follows the order of the template's bindings, while a
signal is read when something reads it. Mixing the two in one tree means a prop
arriving in a different order depending on who wrote the template.

## Running one thing

Every script stands alone:

```bash
./scripts/check-kinds.sh          # just the name chain
./scripts/check-angular.sh        # the whole chain, on hello-angular
./scripts/check-list.sh           # 5,000 rows, and that scrolling creates no views
./scripts/check-macos.sh          # builds a .app, launches it, screenshots it
```

Some take an app:

```bash
./scripts/check-angular.sh examples/controls
```

And the Rust side is ordinary cargo:

```bash
cargo test                  # the whole workspace
cargo test -p an-core       # one crate
cargo test -p an-watch snapshot
```

Host crates compile empty off their own platform, which is what keeps
`cargo test` working on a Mac — and also what means a green `cargo test` does not
prove the iOS host compiles. That is the cross-compilation step at the end of
`check-all.sh`.

## The half that needs hardware

Five scripts are not in `check-all.sh` because they cannot be:

| Script | Why it needs a device |
|---|---|
| `check-a11y-device.sh` | Installs the APK and asks the system for its real accessibility tree. |
| `check-builtins-device.sh` | Haptics, share and the rest, where the hardware is the answer. |
| `check-keyboard-device.sh` | The keyboard's height is the platform's, not the simulator's. |
| `check-measure-device.sh` | Text measurement with the device's real fonts and text-size setting. |
| `check-rotation-device.sh` | Rotation and the insets that come with it. |

They print `skipped` with a reason when nothing is plugged in, and they say
*which* reason — a locked phone says it is locked rather than letting the code
take the blame.

## The accessibility checks are not assertions about intent

`check-accessibility.sh` and `check-a11y.sh` do not check that the props were
set. They read the tree back **from outside the app** — the same API VoiceOver
and TalkBack use — and compare what a screen reader would actually be told.
That is a different claim, and the only one worth making. See
[accessibility](/accessibility/overview/) for what the six props become on each
platform.

## Adding a check

The convention is short and worth following: one subject per script, `ok` and
`FAIL` lines with a plain-English description, `skipped` when the prerequisites
are missing, and a non-zero exit only on `FAIL`. Most of them are twenty lines
of bash around a `grep` over `headless` output.

```bash
OUTPUT="$(cargo run -q -p an-bridge --example headless -- build/bundle/mine/main.js 6 2>&1)"
check 'Switch#[0-9]+ .*on=true' 'the switch arrived on'
```

Then add it to `scripts/check-all.sh`, in the group it belongs to.
