---
title: Android
description: All 25 primitives on android.view, Material 3 resolved without Gradle, and the six controls that measure zero until you give them a size.
sidebar:
  order: 2
---

All twenty-five primitives, on real `android.view.View`s, with Material 3
underneath and no Gradle anywhere.

```bash
an android                      # examples/hello-angular on a connected device
an android examples/controls    # the system controls
an dev --android                # the same, watching, with hot refresh
```

The Android SDK and the NDK are found through `ANDROID_HOME`, `ANDROID_SDK_ROOT`
or their usual location. Material 3 is resolved once:

```bash
python3 scripts/fetch-android-deps.py
python3 scripts/prepare-android-deps.py
```

## The shape of it

The split of work is deliberately different from iOS. There, Rust talks to UIKit
directly, because the Objective-C bridge is cheap and typed. Here every call
crosses JNI, so the surface is kept as small as possible: Java exposes a handful
of methods on one host class and Rust calls those. The Rust crate is about a
thousand lines; the Java host alone is three and a half thousand.

The whole renderer is twelve method signatures — create, destroy, insert,
remove, set prop, set text, set listener, set layout, set content size, set
root, flush, clear — and **every prop travels as a string**, colours included.
The number of distinct types does not justify a JNI signature per type.

Rust holds the `JavaVM` and never a `JNIEnv`, re-attaching per call: everything
runs on the UI thread, so it is cheap. A Java exception is described and cleared
before being reported, because otherwise all you get is "Java exception was
thrown" with no idea which one.

**stderr is redirected into logcat** by a pipe, a `dup2` and a reader thread.
Without it, every `eprintln!` and every panic message vanishes, and a failure
looks like a blank screen.

### Two threads, mirroring iOS

Same 8 MB stack for the same reason, same 12 ms frame budget, same rule against
queueing a tick on top of one still running, same hot-reload semantics. The
clock is a `Choreographer.FrameCallback` re-posted every frame, which is the
`CADisplayLink`'s counterpart.

### The mount model, and the container that measures nothing

A real view hierarchy: one view per node, `addView` for structure, and absolute
frames converted from points to pixels.

The container is a custom `ViewGroup` that **computes nothing**. Letting Android
measure would put two layout engines in the same fight Auto Layout would start
on iOS. Its layout params extend `MarginLayoutParams` and not the plain kind,
because a `ScrollView` is internally a `FrameLayout` and measures children with
`measureChildWithMargins` — with plain params the app crashed on the first
scroll. `onMeasure` measures each child exactly at its frame and reports the
union, because a `ScrollView` measures its child with an unspecified height and
returning the suggested minimum would collapse the content to nothing.

Children are clipped by default: they are absolutely positioned and can land far
outside the parent.

## Material, and what is genuinely hand-drawn

Most of the controls are the real thing — `MaterialSwitch`, Material `Slider`,
`CircularProgressIndicator`, `LinearProgressIndicator`, `MaterialButton`,
`SearchView`, `Spinner`, `Toolbar`, `WebView`, `VideoView`, `EditText`, and real
dialogs for `an-alert` and `an-modal`.

Three that look assembled are not:

| Primitive | What it actually is |
|---|---|
| `an-tab-bar` | A `BottomNavigationView`. The pill behind the selected icon, the change animation, the TalkBack behaviour and the per-version height all come from Material. Only the menu rebuild and the two-colour state list are written here. Android's own platform has no bottom bar — `android.widget` stopped at 2011's tabs. |
| `an-segmented-control` | A `MaterialButtonToggleGroup`, Material 3's segmented button as it ships. End shapes, the selected container, the check mark and the press response are the library's. |
| `an-stepper` | Composed, not drawn: two Material icon buttons and a Material text view. Material 3 has no stepper — it is not missing from the library, it is absent from the design system — so it is assembled out of pieces that *are* Material rather than by drawing an imitation. It shows the value, unlike iOS's `UIStepper`, because on Android two loose buttons do not say what they change. |

Two things really are drawn by hand, and both say so:

- **`an-map-view` is OpenStreetMap tiles on a `Canvas`.** Android ships no map
  in the platform; Google's lives in Play Services behind an API key and a
  Gradle dependency. So: 256-pixel tiles from `tile.openstreetmap.org`, a
  four-thread fetcher, an LRU cache sized at a quarter of app memory, the
  Mercator projection written out, and dragging handled in `onTouchEvent`. A
  one-pixel sentinel bitmap marks a tile already in flight, so dragging does not
  re-request it. It is a real native view — not a hidden browser — but it is not
  the system's map either: no routes, no search, no blue dot.
- **Pull-to-refresh.** The scroll view detects the downward drag at the top and
  draws the spinner arc itself, because `SwipeRefreshLayout` is a separate
  AndroidX dependency. The threshold is 72 dp.

**Icons come from a bundled font**, not from system drawables: Material Symbols
as a TTF plus its codepoint map, looked up by name. `android.R.drawable` has
been frozen since 2011 for compatibility and is not Material 3's set. Where the
system wants a `Drawable` rather than a view, the glyph is rendered onto a 24 dp
bitmap.

Nothing is unsupported on a phone. The machinery that refuses a primitive exists
and is only consulted on a watch — see [Wear OS](/platforms/wearos/).

## The back button

`onBackPressed` asks the host, which sends `back` to the **last** node that
subscribed — the top of the stack — and returns true. If nothing is listening it
falls through to the system and the app exits.

The host only reports; undoing the navigation is the router's job. That is the
same contract as the iOS edge gesture.

## Insets

The insets path reads the root window insets, takes system bars and display
cutout together, divides by density, compares against the last four values, and
only on a real change dispatches them.

**Four numbers do not fit in a positional event**, so they travel as JSON and
Rust parses them by hand, without a JSON parser, precisely so the event that
reaches the template is identical to the one iOS sends. The same API is used a
second time to add the gesture strip's height to the measured tab bar.

## Material 3 without Gradle

Two Python scripts stand in for the dependency resolver.

The first downloads Material and everything it drags in, resolving POMs by hand
against Google's and Maven Central's repositories. It follows
`dependencyManagement` including BOM imports — without that, androidx media ends
up with no version and is never downloaded — and resolves Maven version ranges
to their lower bound. Before that, anything with brackets in its version was
discarded, which silently dropped half of androidx until the app started and
could not find a class. Conflicts go to the first one seen, nearest the root,
the way Gradle does. A handful of artefacts are excluded on purpose: since
Kotlin 1.8 the split standard-library jars are inside the main one, and shipping
both sets makes the dexer refuse.

The second unpacks each `.aar`, collects the jars, compiles each library's
resources once, and writes three lists — a classpath, a resource set and a
package list — rebuilding only what is missing, because fifty libraries'
resources take time and never change.

At link time that becomes `--extra-packages` (one `R` class per library, whose
ids therefore cannot be constants), `--non-final-ids`, and `--auto-add-overlay`,
since the libraries' resources overlap on purpose and would otherwise be an
error. The shell's own resources go last so they can override.

## The keyboard

A text input is a bare `EditText` with no padding and no background. What
configures the keyboard is one integer, and all four props are flags of it —
keyboard kind, capitalisation, autocorrect, secure entry — so the whole integer
is **recomposed from stored state** whenever any one of them arrives. Applying
one alone would erase the other three.

Secure entry wins over the keyboard variant, because a masked field with an
email keyboard would show the text. Capitalisation and no-suggestion flags are
skipped on numeric keyboards. And setting the input type resets the typeface to
the password monospace, so the typeface has to be applied again right after.

## Text measurement

A `StaticLayout` on a shared paint, with the width taken as the widest line and
the height as the layout's. Both come back packed into a single `long` as
hundredths of a point: two JNI calls per measurement would cost double for
nothing.

**Font weight is binary here**: 600 or more is bold, anything else is regular.
That is much coarser than the nine-step map the Apple hosts use, and it is worth
knowing when a design leans on 500 or 300.

The cache lives in Rust rather than Java, keyed the same way as the Apple ones,
and for the same reason plus one more: every query here crosses JNI, which is a
good deal more expensive than an Objective-C message send. Infinite width
travels as `-1`, because JNI has no option type.

## What is missing

- **Six controls measure zero.** The natural size of a control is measured by
  building a throwaway probe — but only six names are handled: switch, slider,
  activity indicator, progress bar, button and tab bar. The core asks for
  thirteen. So `an-segmented-control`, `an-stepper`, `an-search-bar`,
  `an-select`, `an-date-picker` and `an-navigation-bar` all measure **0×0**, and
  are invisible unless the template gives them a size. `an-icon` is in the same
  position and escapes it only because its directive pins width and height from
  `[size]`. **No warning is emitted for this**, which makes it the sharpest
  thing on this page.
- **`lineHeight` and `letterSpacing` are drawn but not measured.** Both are
  applied when rendering and neither is passed to the measurement, so layout
  reserves the wrong box. iOS at least measures `lineHeight`.
- **No keyboard inset below API 30.** From API 30 the IME arrives like any
  other inset and it arrives *moving*: `WindowInsetsAnimation.Callback` gives
  it once per frame, so the form travels with the keyboard. Before that,
  `WindowInsets.Type.ime()` does not exist and neither does the callback, so
  nothing arrives and a field at the bottom stays under the keyboard. The old
  trick — watching the window's visible frame shrink — only reports anything if
  the window is allowed to resize, and this shell asks it not to precisely so
  that the layout the core computed is the one that gets drawn.
- **Dark mode is forced process-wide**, and the code says it should not be:
  appearance is the app's decision, not the shell's.
- **Back is the deprecated callback.** There is no predictive back.
- **Only `arm64-v8a` is built.** No x86-64 emulator image and no 32-bit ABI.
- **`flush` runs twice per productive frame** — once from the core's mount side
  and once from the activity — so the container is asked to lay out twice and
  the dialogs are reconciled twice.

Warnings, none of them silent: the crown on a phone (once); a slider step size
that does not divide the range, which makes Material crash while drawing, so it
is reported and the slider stays continuous; an accessibility state that is not
an object; a `checked` that is neither boolean nor `'mixed'`; an unknown role,
which exists because reaching it means the TypeScript role list and the host's
have drifted apart; `expanded` on a view with no `(press)`, since on Android
expanding is an *action* and without one a reader would announce something that
cannot be done. What is set and what is refused is in
[Accessibility on Android](/accessibility/android/).

## Into Google Play

```bash
an android --sign --release      # a release-signed APK
an android --aab --release       # the bundle Play takes
```

The debug keystore this page's build uses is the one Android Studio generates,
with the password written into the source; no store accepts it. `--sign` uses a
keystore you generate and keep, `--aab` builds an Android App Bundle — which is
the only thing Google Play has taken since August 2021.

There is no Gradle for this either. `aapt2 link --proto-format` produces the
protobuf manifest and resources a bundle wants, the module is assembled by hand,
`bundletool` turns it into the `.aab` and `jarsigner` signs it, because
`apksigner` refuses one. `bundletool` is not part of the Android SDK —
`scripts/fetch-android-deps.py` brings it.

This whole path runs end to end in `scripts/check-signing.sh`, with a keystore
the check generates: an upload key needs nobody's permission. What that check
cannot do is upload anything, and neither can `an`. What to get from Google, and
where to put the keystore, is in
[Signing and distribution](/guide/signing-and-distribution/).

## Build and run

```text
cargo build --target aarch64-linux-android -p an-android
aapt2 compile / link  ·  javac  ·  d8  ·  zip  ·  zipalign  ·  apksigner
→ build/android/<AppName>.apk
```

`javac` runs with `-source`/`-target 17` rather than `--release`, because with
`--release` it ignores the boot classpath and the compilation has to go against
`android.jar`. The dexer splits into several dex files once Material is in
there, and all of them go into the zip. Only the manifest comes out of `aapt2`,
so the dexes, the shared library and the assets are zipped in afterwards.

**Signing is debug-only**: the standard debug keystore, created with `keytool`
if it is not there. There is no release identity here.

The app id can differ from the shell's package: the manifest package is renamed
at link time while the classes stay put, which is why launching qualifies the
activity as `<applicationId>/dev.angularnative.MainActivity`.

**`${applicationId}` in the manifest is substituted**, and it is the only
placeholder there is. Renaming the package rewrites the package and qualifies
relative class names; it leaves every attribute value exactly as it found it,
and one attribute cannot survive that. A `<provider>` authority is unique
across the whole **device**, so the shell's `FileProvider` — the one `share`
hands files through — cannot carry a fixed name: two angular-native apps would
both claim it and the second to be installed would fail with
`INSTALL_FAILED_CONFLICTING_PROVIDER`. It is written
`${applicationId}.anfiles` and comes out as the app's own.

A plugin's `<uses-permission>` and `<uses-feature>` entries are merged into a
copy of the manifest, never into yours. A duplicate permission is skipped —
asking twice is asking once — and a feature the app already declares with a
different `required` keeps the app's, with a log.

**A physical device works.** Devices are listed with `adb` and classified by
asking each one its build characteristics; exactly one of the right shape is
required, and `--device` on `an wearos` is refused if the shape is wrong. Every
`adb` call carries `-s`: without it `adb` refuses to act the moment two devices
are attached — and if the other one is unauthorised it does not even refuse, it
sends the watch's APK to the phone.

One emulator assumption remains: `an dev --android` bakes `10.0.2.2` into the
app as the server's address, so hot refresh against a real handset does not
reach it. The dev client **long-polls over HTTP** rather than using a WebSocket,
because the platform ships no WebSocket client and pulling in a whole HTTP
library for this is not worth it.
