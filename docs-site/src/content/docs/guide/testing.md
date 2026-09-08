---
title: Checking it without a device
description: One command runs the lot, `headless` renders a bundle with no platform under it, and a handful of scripts guard the lists that are duplicated across three languages.
sidebar:
  order: 10
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

## What did not run

A `--` line in the middle of two thousand is easy to miss, so the run ends with
a `== skipped` section that counts them and repeats each one:

```
== skipped
       cross-compilation skipped: the nightly toolchain is missing
       the signed macOS build is skipped: this keychain has no codesigning identity
  --   2 steps did not run; the reason is printed where each one happened.
```

`ok   nothing was skipped: every step ran` is the other outcome. It is a report
and not a gate — `--no-android`, a machine with no simulator runtime and a
keychain with no signing identity all skip for good reasons, and a suite that
failed on any of them would stop being runnable on a laptop.

The one skip that has no good reason is the tier-3 Apple cross-compilation.
`aarch64-apple-tvos-sim`, `aarch64-apple-visionos-sim` and
`aarch64-apple-watchos-sim` ship no prebuilt `std`, so `-Z build-std` compiles it
from `rust-src`, which is nightly-only; `rust-toolchain.toml` pins stable,
because a toolchain file pins one channel and the other seven crates have no
business on nightly. One command covers all three, and it is the one CI runs:

```bash
rustup toolchain install nightly --component rust-src
```

Which is why the CI job greps its own output for `cross-compilation skipped` and
fails on it. The runner installs that toolchain; a skip there is not a machine
short of a toolchain, it is three targets nobody is building.

## The two halves

```bash
./scripts/check-all.sh               # everything
./scripts/check-all.sh --no-android  # everything except the half below
./scripts/check-android.sh           # the Java shell, Wear OS, the two Android cross-compilations
```

The Android half is a script of its own because it wants a different machine
from the rest: the Apple checks need swiftc, the simulators' SDKs and a real
`.app`, and the Android ones need the NDK and nothing Apple. `check-all.sh`
calls `check-android.sh` rather than repeating what is in it, so there is one
list of each half and neither can drift from the one CI runs.

## In CI

`.github/workflows/ci.yml` runs those two commands on two runners: the Apple
half on `macos-15`, the Android half on `ubuntu-24.04`. Both runner images carry
the Android SDK and the NDK, so the Mac could do the lot — it does not because a
macOS minute bills at ten times a Linux one, and nothing in the Android half
needs a Mac.

The Mac job installs one thing before anything else: `rustup toolchain install
nightly --component rust-src`. The iOS and Android targets come from
`rust-toolchain.toml`; nightly cannot, and that step is the only reason the three
tier-3 Apple cross-compilations run anywhere. The job then greps the suite's own
output for `cross-compilation skipped` and fails on it, so the day that install
stops working the run goes red instead of quietly dropping three targets.

Three things the Linux job has to get right, none of them a guess about the
runner:

- **The NDK** is already on the image, so there is no `sdkmanager` step. The
  image sets `ANDROID_NDK_HOME` to its default, which is what keeps `an` off the
  newer NDKs also installed. Before compiling anything the job asserts that the
  compiler `an env android` names actually exists — an NDK that dropped the API
  level the core is built against otherwise shows up as a missing linker inside
  an unrelated crate.
- **The host tag.** `find_ndk_toolchain` reads whatever single directory is
  under `toolchains/llvm/prebuilt/` instead of assuming `darwin-x86_64`, so it
  finds `linux-x86_64` with nothing changed.
- **Material 3**, which `fetch-android-deps.py` resolves by hand: a hundred-odd
  round trips to two Maven repositories for 43 MB, on every run, even when every
  jar is already on disk. `vendor/android/` is cached, keyed on the two scripts
  that decide what goes into it and on the build-tools version whose `aapt2`
  compiled the resources — the three things that change its contents.

A fourth job boots an emulator and runs `check-a11y-device.sh` on it, with
`AN_ABI=x86_64` so the APK carries the architecture the emulator is. It is
`continue-on-error` and does not gate `main`: it is the only check that reads
the accessibility tree back out of a real Android, and also the least
trustworthy signal in the file — a hosted emulator that fails to boot would
otherwise turn the run red for something that is not in the code, and a red run
nobody believes is worse than no run.

:::caution[The workflow has never executed]
Every step of it has been run by hand on a Mac — `an env android` with a
`linux-x86_64` toolchain laid out on disk, a cold dependency fetch, the Java
shell, Wear OS, both cross-compilations, an `x86_64` APK — but the file itself
has never run on a GitHub runner. What nobody has watched: the first cache miss,
a real Linux host, and the emulator booting.
:::

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
