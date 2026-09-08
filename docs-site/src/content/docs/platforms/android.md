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
or their usual location — and `ANDROID_NDK_HOME` for an NDK that is not under
`<sdk>/ndk`. Nothing about them is committed: the version and the host tag
(`darwin-x86_64`, `linux-x86_64`) are read off the disk, and the linker, the
archiver and bindgen's sysroot go to `cargo` as environment. `.cargo/config.toml`
carried all of it written out until recently, which meant an Android build
worked on the machine of whoever wrote the file and on no other.

Material 3 is resolved once:

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

Android has two of them and the shell answers both, because `minSdkVersion` is
24 and the second one arrived in 33.

On API 24 to 32 it is `onBackPressed`, deprecated since 33 and still the only
back those levels have. From 33 it is an `OnBackInvokedCallback` on the
activity's `OnBackInvokedDispatcher`, which the manifest opts into with
`android:enableOnBackInvokedCallback`. The opt-in is not decoration: API 36
removed the opt-out, so an app that never opted in gets neither callback and
back stops working altogether. Measured on an Android 16 phone, back on a
pushed screen left the app instead of popping it.

Either path asks the host, which sends `back` to the **last** node that
subscribed — the top of the stack. The host only reports; undoing the
navigation is the router's job. That is the same contract as the iOS edge
gesture.

**Predictive back** is the API 34 half of the callback,
`OnBackAnimationCallback`. The gesture says where it has got to and the host
puts the two screens where a pop would have them at that fraction: the top
sliding out to the right, the one underneath coming back from the third of a
width it rests at. Letting go continues that one movement instead of starting a
second; letting go early puts both back.

The callback is registered **only while something is listening**. One that
stays on the dispatcher tells the system the app will handle every back, and
the system then draws no preview of its own — an app with no stack on screen
would lose the back-to-home animation every other app has.

One thing follows from the host reporting rather than deciding: a stack goes on
listening at its own root, so back on the first screen reaches the router,
finds nothing to go back to and does nothing. It does not leave the app.

## Insets

The insets path reads the root window insets, takes system bars and display
cutout together, divides by density, compares against the last four values, and
only on a real change dispatches them.

**Four numbers do not fit in a positional event**, so they travel as JSON and
Rust parses them by hand, without a JSON parser, precisely so the event that
reaches the template is identical to the one iOS sends. The same API is used a
second time to add the gesture strip's height to the measured tab bar.

**The keyboard goes into the bottom inset** rather than into an event of its
own, and how it gets there depends on the level. From API 30 it is
`WindowInsets.Type.ime()`, read once per frame of the keyboard's own animation.
Below 30 neither that type nor the animation callback exists, so the manifest
asks for `adjustResize` and the host measures instead: on every layout pass it
compares the container's bottom edge against the window's visible frame and
reports the overlap. When the window really did resize the overlap is zero —
the container already ends where the keyboard starts, and the viewport listener
has redone the layout for the smaller size — so reporting the keyboard's height
on top of that would move every form twice.

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

**Font weight is nine steps from API 28 and two below it.**
`Typeface.create(family, weight, italic)` takes CSS's number since API 28, so
from there 300 and 500 are weights of their own, the way they are on the Apple
hosts. The shell's `minSdkVersion` is 24, and on 24 through 27 the platform has
nothing that takes a number: the typeface carries bold or not-bold and nothing
else, so the scale collapses at 600 — 100 to 500 draw as regular, 600 to 900 as
bold. A design leaning on 500 for a heading gets medium on anything current and
regular on an Android 7 phone.

Both halves collapse in the same place because both go through the same
`typefaceFor`: the weight the `StaticLayout` measures is the weight the
`TextView` draws, and on the old branch the measurement mirrors the synthetic
bold `TextView.setTypeface(tf, style)` applies to a family with no bold cut,
which is wider than the regular it is faked from. `check-android-java.sh`
proves the count without a device — it reflects into the `AnHost` bytecode the
APK just got, with `Build.VERSION.SDK_INT` and `Typeface` stubbed, and asserts
nine faces at 34 and two at 24.

The cache lives in Rust rather than Java, keyed on everything the answer
depends on — the text, the font, the spacing, the line height, the width limit
— for the reason the Apple hosts have one plus one more: every query here
crosses JNI, which is a good deal more expensive than an Objective-C message
send. Infinite width travels as `-1`, because JNI has no option type.

Line height and letter spacing are measured and not only drawn. Both were
applied to the `TextView` and neither crossed JNI, so the layout reserved a box
for text without them and the host drew the text with them: positive spacing
ran past its box or wrapped a word early, and a `lineHeight` above the font's
own was reserved by nobody, so the lines ran into whatever came after them.
They travel as two more floats on the same call. `Paint.setLetterSpacing` wants
ems where the core carries points, so the value is divided by the text size —
the same division the drawing side does, and the reason the spacing has to be
reapplied whenever the size changes. The line height becomes
`setLineSpacing(lineHeight - fontHeight, 1f)`, which is all `setLineHeight` is,
with the same clamp at zero the host uses below API 28 where there is no line
height at all, only what is added to the font's own. No line height travels as
`-1`, the way an infinite width does.

## What is missing

- **The keyboard does not travel with its animation below API 30.** From API 30
  the IME arrives like any other inset and it arrives *moving*:
  `WindowInsetsAnimation.Callback` gives it once per frame, so the form travels
  with the keyboard. Below 30 there is no such callback and no
  `WindowInsets.Type.ime()` either, so the window is resized instead —
  `adjustResize` — and the layout is redone once at each end rather than per
  frame. The field does get out of the way; it just arrives in one step. See
  [Insets](#insets).
- **Font weight is two steps below API 28.** `Typeface.create(family, weight,
  italic)` takes CSS's number from API 28 on; the shell's `minSdkVersion` is 24
  and on 24 through 27 nothing in the platform takes it, so 100 to 500 draw as
  regular and 600 to 900 as bold. The measurement collapses in the same place,
  so the box still fits what is drawn. See [Text measurement](#text-measurement).
- **No system bar icon colour below API 30.** `WindowInsetsController` is not
  there before that, so the status and navigation bar icons keep the theme's and
  a light bar over a dark app can be hard to read. It is said once rather than
  skipped in silence. See [What the app looks like](#what-the-app-looks-like).
- **Three outputs the base directive declares and this host does not deliver**:
  `(hover)`, `(crown)` and `(crownIdle)`. Android does send hover events — under
  a mouse, under a stylus, on a Chromebook — but `[cursor]`, the prop that goes
  with `(hover)`, has no meaning here, and half a pair that works on a
  Chromebook and never on a phone gets tested once and shipped broken. The crown
  is the watch's: `(crown)` and `(crownIdle)` do arrive on Wear OS and are
  turned down on a phone. Each is refused when the template subscribes, once,
  with its reason — and so is an output put on a primitive that does not report
  it, `(scroll)` on an `<an-view>`, which is answered naming the widget the node
  actually mounted.
- **Back at the root of a stack does not leave the app.** The stack subscribes
  to `back` for as long as it is on screen, so the host answers every press and
  the router then finds nothing to pop. See
  [The back button](#the-back-button).
- **No `armeabi-v7a` unless you ask for it.** `--abi armeabi-v7a` builds it;
  nothing defaults to it. See [Which ABIs](#which-abis).

Warnings, none of them silent: the crown on a phone (once); a slider step size
that does not divide the range, which makes Material crash while drawing, so it
is reported and the slider stays continuous; an accessibility state that is not
an object; a `checked` that is neither boolean nor `'mixed'`; an unknown role,
which exists because reaching it means the TypeScript role list and the host's
have drifted apart; `expanded` on a view with no `(press)`, since on Android
expanding is an *action* and without one a reader would announce something that
cannot be done. What is set and what is refused is in
[Accessibility on Android](/accessibility/android/).

## What the app looks like

`app.appearance` in `angular-native.json` takes `system`, `light` or `dark`,
and `system` is the default: light phone, light app.

```json
"app": { "name": "MyApp", "bundleId": "com.example.myapp", "appearance": "system" }
```

`an` writes it into the merged manifest as a `<meta-data>` entry and the
Activity reads it back before `super.onCreate` — after it, `AppCompatActivity`
has already read the night mode and would recreate itself on the first frame.
What follows it is Material: dialogs, date pickers, the text selection handles.
It is **not** what paints the screen; that is the app's own background, so an
app that follows the device has to paint with it.

This used to be forced. The shell called `setDefaultNightMode(MODE_NIGHT_YES)`
in a static block, on every app built with it, and the reason was real: with
the system in light mode a white navigation bar came out underneath a screen
the app had painted dark. That cured a symptom belonging to the bars by taking
the choice away from everybody.

The bars are handled where they live now. The window already draws edge to
edge, so they are transparent and what shows behind them is the app's own
background; their icons are chosen from that background's luminance — the
sRGB relative one, because the eye is some seven times more sensitive to green
than to blue and a saturated blue that averages "light" reads as dark. Below
API 30 there is no `WindowInsetsController` and the icons keep the theme's,
which is said once rather than silently skipped.

## Into Google Play

```bash
an android --sign --release      # a release-signed APK
an android --aab --release       # the bundle Play takes
```

The debug keystore this page's build uses is the one Android Studio generates,
with the password written into the source; no store accepts it. `--sign` uses a
keystore you generate and keep, `--aab` builds an Android App Bundle — which is
the only thing Google Play has taken since August 2021.

### Which ABIs

```bash
an android                                  # arm64-v8a
an android --abi x86_64                     # an emulator on an Intel machine
an android --abi arm64-v8a,x86_64           # both, in one APK
an android --aab --release                  # arm64-v8a and x86_64
```

An APK carries `arm64-v8a` and nothing else. It is one file that has to hold
every architecture it might ever be installed on, and each one is another
cross-compilation of the core with QuickJS inside it — a cost the dev loop pays
between saving a file and seeing it, for a device whose architecture does not
change.

A bundle carries `arm64-v8a` **and** `x86_64`. Play splits a bundle by ABI and
installs one slice, so the second target costs no user a byte at download time;
leaving it out costs the listing every x86-64 device there is — Chromebooks,
and the emulator on every Intel machine, which is what a reviewer or a tester
reaching for the app from a desktop is running. Play does not warn about this.
The app is simply not offered there.

`armeabi-v7a` is in neither default. 32-bit-only devices are under 1% of the
ones in use, and what Play requires is that a 64-bit build *exists*, not that a
32-bit one does — so it would be a cost on every build for an audience almost
nobody has. `--abi armeabi-v7a` is there for whoever does.

`--abi` overrides the default outright rather than adding to it, so
`an android --aab --abi arm64-v8a` is how you get a bundle with one ABI in it.
Each ABI wants its Rust target installed; `an` says which ones are missing
before it compiles anything, in one message, rather than stopping halfway
through the second one.

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

Hot refresh reaches a real handset. The address baked into the app used to be
`10.0.2.2` — the host machine seen from inside the emulator, and nothing at all
from anywhere else — so a phone over USB polled somewhere that was not there
and said nothing, because nothing had failed. `an dev --android` now opens the
port on the device with `adb reverse` and bakes plain `127.0.0.1`, the same
address every other target uses. A device that turns the reverse down still
gets the app, and is told that saving will change nothing on screen.

The dev client **long-polls over HTTP** rather than using a WebSocket, because
the platform ships no WebSocket client and pulling in a whole HTTP library for
this is not worth it.
