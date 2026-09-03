---
title: Wear OS
description: Angular on an Android watch — the same host as the phone, a round screen, the rotary crown, and the seven primitives that do not belong on a dial.
sidebar:
  order: 7
---

Angular on the Android watch: same core, same layout, same bundle and — this
time — **the same host**.

```bash
an wearos examples/hello-wear   # once, on a watch emulator
an dev --wearos                 # the same, watching, with hot refresh
./scripts/check-wearos.sh
```

## Why this looks nothing like the Apple watch

[watchOS](/platforms/watchos/) has no `UIView` hierarchy, which is why it needed
a declarative host of its own: the Rust tree is mirrored into a model SwiftUI
redraws.

None of that applies here. A Wear OS watch **is** Android: `Activity`,
`Choreographer`, `android.view.View`, `ViewGroup.addView`, `setBackground`. The
host that already existed — `crates/an-android` plus the Java shell — works as
it is. Grep `crates/an-android/src/` for `wear`, `crown`, `rotary` or `watch`
and you find nothing: there is no Wear-specific Rust anywhere. The same crate,
the same `aarch64-linux-android`, the same `dev.angularnative.MainActivity`.

And it works literally: the first thing tried was installing the phone APK,
untouched, on a Wear OS 5 emulator. It started, evaluated the bundle and
painted. So the expensive part — QuickJS, the binary bridge, taffy, the views —
did not have to be done. The watch's work is elsewhere, and it is four things.

## 1. The watch APK is not the phone APK

Even though the phone one starts. What makes this a watch app is one line of the
manifest:

```xml
<uses-feature android:name="android.hardware.type.watch" android:required="true" />
```

Without it, to the system and to the store this is a phone app that happens to
have been installed on a watch. It is the failure you cannot see: it installs
the same, starts the same and paints the same. You only find out when you
publish.

And since a whole file is what changes — `aapt2` knows nothing about variants or
conditionals — the watch manifest is a separate file,
`shells/android/AndroidManifest.wear.xml`, and `an wearos` chooses which one it
links. A project of your own overrides it by dropping an
`AndroidManifest.wear.xml` into its `android/` platform directory. That
directory is created by `an add android`, with the phone manifest only:
**`an add wearos` does not exist**, because the Wear build reuses the Android
platform directory.

The APK gets a different filename too — `<app>-wear.apk` — because both carry
the same package and a shared name would clobber.

The theme also changes, and that is why it moved out of the manifest into
`res/values/styles.xml`: the watch one needs `windowSwipeToDismiss`. Wear OS has
no back button; you drag from the left edge. The system's Wear themes have it,
Material 3's — which are a phone's — do not, and without it the app has no way
out and raises no error: nothing simply happens when you swipe.

`Theme.DeviceDefault`, which is what Google recommends for a view-based Wear
app, cannot be used: `MainActivity` is an `AppCompatActivity`, and AppCompat
refuses to start on a theme that does not descend from `Theme.AppCompat`. So
`Theme.AngularNative.Wear` extends `Theme.Material3.Dark.NoActionBar` — Dark
fixed rather than DayNight, because the screen is OLED and that is half the
battery — with a black window background.

The host does not trust the manifest at runtime, either. It asks the system —
`PackageManager.FEATURE_WATCH` — because the phone APK can be installed on a
watch, and then the manifest lies. Roundness is a separate question,
`Configuration.isScreenRound()`, deliberately independent: square watches exist.

## 2. The screen is round

It is what changes most and what cost the least code, because there was already
somewhere to put it.

On a dial the corners do not exist. Whatever you put there is not clipped with a
warning: it is not painted, and nobody says anything. That is exactly the
question an iPhone notch asks — "how far can I paint?" — and `an-safe-area`
already answers it on the other platforms, so the curve's margin travels through
that and not through a new event. A template has no business knowing whether the
margin set aside for it comes from a notch or from an arc.

What the system gives and what has to be deduced, which are not the same thing:

- **The system gives:** that the screen is round, and the chin on the watches
  that have one, which does arrive as `WindowInsets`.
- **Deduced:** how far to stay clear. The side of the square inscribed in a
  circle of diameter `d` is `d/√2`, so `d·(1 − 1/√2)` is left over, split
  between the two sides: **0.146447 of the diameter per side**. That is the same
  figure `androidx.wear`'s `BoxInsetLayout` uses.

`BoxInsetLayout` is not used directly, and not to avoid pulling in the
dependency: it is a `ViewGroup` that positions its own children, and here taffy
owns the layout. That would be two layout engines deciding the same thing, which
is precisely what the watchOS host avoids.

The insets are read from `WindowInsets` — `systemBars() | displayCutout()` —
and the round inset is then `max`ed into all four edges, so a chin and an arc do
not cancel each other out. The result is dispatched as the same `safeArea` event
with `{top, right, bottom, left}`.

On a 454 px emulator at density 2 that is 227 points of diameter and 33.2 of
margin per side: the usable square is 160×160.

## 3. The crown

The Wear OS rotating crown **is not a touch**. It arrives as `ACTION_SCROLL`
from `SOURCE_ROTARY_ENCODER`, down the generic event path —
`onGenericMotionEvent` — and not the touch one. That is why the host could not
see it: `AnScrollView` only looked at `onTouchEvent`.

Three things that are not obvious:

- **The value is in wheel notches, not pixels.** `AnScrollView` converts them
  with `ViewConfiguration.getScaledVerticalScrollFactor()`, the same factor a
  mouse uses, and negates the sign — turning the crown up is positive, and
  scrolling down increases `scrollY`.
- **The events go to the view that has focus.** A list does not ask for it on
  its own: without `setFocusableInTouchMode(true)` the system sends them to
  whoever holds focus — usually nobody — and the crown does nothing, with no
  error at all.
- **That is only switched on on a watch.** On a phone, a list claiming focus
  takes it away from whatever text field was underneath.

Nothing is declared from the template. `an-scroll-view` listens for it, and
`(scroll)` comes out exactly as if a finger had dragged, so `an-virtual-list` —
which sits on the same scroll — is driven by the crown without being touched.

### And raw, for what is not scrolling

Scrolling is what you almost always want, but not always: turning up a volume,
moving an hour, changing screen. That is `(crown)`, the same output the Apple
watch uses, delivered here by the Android host:

```html
<an-view (crown)="turned($event)" (crownIdle)="stopped()"> … </an-view>
```

| Key | What it is |
|---|---|
| `delta` | Notches turned since the previous event. |
| `offset` | Accumulated since this turn began. |
| `velocity` | Notches per second, signed. |

Four differences from the Apple watch, all four the platform's:

- **Notches travel, not points.** What the system gives is a mouse wheel's axis;
  converting it to pixels is the scroll view's job, and doing it here as well
  would invent a scale the template did not ask for.
- **`offset` counts from when the turn started**, not from when the view took
  focus. On watchOS focus is visible — there is a highlight; on Android a view
  that takes focus does not change appearance, so nobody could see that origin.
- **`(crownIdle)` is counted by the host**, because the system sends no end: it
  sends notches and goes quiet. A quarter of a second of silence is the cut.
  `velocity` is 0 for the first notch of a turn rather than a huge number.
- **On an `an-scroll-view` the crown does both.** The listener is consulted
  before the scroll and `onGenericMotion` returns false, so merely listening
  does not freeze the list. `examples/hello-wear` shows the points scrolled and
  the notches turned at the same time, from the same turn.

Off a watch, `(crown)` says so when you subscribe and never fires: a phone has
no wheel to turn, and an output that never fires with nobody saying so is worse
than not having it.

What it does **not** do today: move an `an-slider` or an `an-stepper` by itself.
The crown reaches the template and the template can move the value; what there
is not is a control that takes it from the system, the way watchOS's focused
`Slider` and `Stepper` do.

## 4. What does not belong on a watch

When the host finds it is on a watch, seven primitives are not mounted. In their
place goes a visible marker with the tag's name, in red at 11 dp, and an error
in the log with the reason.

| Primitive | Why not |
|---|---|
| `an-tab-bar` | Wear OS has no tab bar: you navigate by swiping and with the crown, not with tabs along the bottom. |
| `an-segmented-control` | A segmented control does not fit across a dial. |
| `an-navigation-bar` | The top of a watch is the system clock, and "back" is the edge swipe. |
| `an-search-bar` | Searching on a watch is not a field inside the screen but the system's input screen — dictation, scribble or keyboard. |
| `an-web-view` | Wear OS ships no WebView: no package in the system implements `android.webkit`. |
| `an-date-picker` | The platform picker is a phone calendar; on a watch a date is picked full screen. |
| `an-select` | A dropdown anchors a menu, and a dial has nowhere to anchor it. |

The marker keeps to itself: its id goes into a set that the property and text
paths consult, so it does not take the `[title]` or the `[color]` of the
primitive it replaced. If it did it would be dressed as that primitive — it is a
`TextView`, so a label and a colour would go straight in — and it would look
like the control had worked. `check-wearos.sh` checks that the list in Java and
the list on this page say the same thing.

**The two watches differ**, and the difference is worth knowing if you write for
both: Apple's supports `an-select` and `an-date-picker` and cannot do
`an-textarea`, `an-map-view` or `an-video-view`; Wear excludes `an-select` and
`an-date-picker` and keeps all three of those.

What does work, and why it is not in the list: `an-switch`, `an-slider`,
`an-button`, `an-progress-bar`, `an-activity-indicator`, `an-icon`,
`an-stepper`, `an-alert` and `an-modal` are Material controls that draw the same
on any screen; `an-text-input` opens the system keyboard, which on a watch is
Wear's input screen; and `an-video-view` and `an-map-view` mount exactly as they
do on a phone — the map is not even the system's, it is OpenStreetMap tiles on a
`Canvas`.

Of that list, `an-switch`, `an-slider`, `an-progress-bar`, `an-icon` and
`an-button` have been seen running on the emulator, with the Material look they
have on a phone. `an-video-view` mounts and requests audio focus, but the
emulator's Wear image ships no codecs — "OMX service is not available" — and the
player ends in `error (100, 0)`. That is the emulator, not the watch, but until
it is tried on a real one nothing else can be said.

## Testing it without a watch

There is no Wear AVD by default. Create one:

```bash
sdkmanager "system-images;android-34;android-wear;arm64-v8a"
avdmanager create avd -n an-wear \
  -k "system-images;android-34;android-wear;arm64-v8a" -d wearos_large_round
```

Two things in its `config.ini` need changing — `avdmanager` leaves both at "no":

```ini
hw.lcd.circular=true    # without this isScreenRound() is false and there is no round inset
hw.rotaryInput=yes      # without this there is no crown to turn
```

Start the emulator with its port set. That is not a habit: `adb` identifies the
device by it — `emulator-5560` — and with a phone running at the same time, an
`adb` without `-s` does not know which one you meant.

```bash
emulator -avd an-wear -port 5560 -no-boot-anim
```

The crown is turned from the terminal, which is how the screenshots were taken:

```bash
adb -s emulator-5560 shell input rotaryencoder scroll --axis SCROLL,-3   # down
adb -s emulator-5560 shell input rotaryencoder scroll --axis SCROLL,3    # up
```

The value is in notches and one notch is a lot: with the system's scroll factor
at density 2, `SCROLL,-1` moves about 43 points, more than a quarter of the 160
usable ones. To see the scrolling from the inside — rather than just landing at
the end of the list — fractions work: `SCROLL,-0.2` is about 9 points.

The Wear emulator goes back to the watch face after ten seconds untouched, like
a real watch. To keep it still while you look:

```bash
adb -s emulator-5560 shell settings put system screen_off_timeout 1800000
adb -s emulator-5560 shell svc power stayon true
```

`an wearos` picks the device by asking its shape — `ro.build.characteristics`
contains `watch` on any Wear OS image — so with a phone and a watch running at
once each APK goes to its own, and `--device` names one by adb serial. A
`--device` that is not a watch is refused rather than installed: the watch APK
installs onto a phone without complaint, which is the kind of failure you only
see once you publish.

## Hot refresh

`an dev --wearos` assembles the watch APK — not the phone one sent somewhere
else — pushes it to the device shaped like a watch, and keeps watching. On save
the new bundle is stitched over the running one: the change shows without losing
the screen or the state. Its `--device` is an `adb` serial, not a simulator
name.

The emulator sees the Mac at `10.0.2.2`, the same as the phone one, so the
server does not change. Unlike the Apple shells, which use a WebSocket, the
Android shell long-polls the dev server.

The first thing it did was leave the screen black, and out of that came two bugs
this document could not have found:

- **A node that moved was not unregistered from its previous parent.** When
  `an-safe-area` becomes a view after its children are already mounted — which
  is exactly what happens when the tree is rebuilt hot — the removal was never
  sent. UIKit and AppKit move a view that already has a parent without saying
  anything, so it was never noticed on iOS or the desktop; `ViewGroup.addView`
  throws, and the subtree was left unmounted. The core no longer accepts that
  insert.
- **After a refresh, templates were left with no directives.** The refresh
  emptied the definition's directive list expecting Angular to recompute it, and
  Angular never recomputes it. It went unseen because a primitive input that
  matches nothing still arrives as a property; what does not arrive is what
  makes something a component rather than a property, and here that is
  `an-safe-area`, which stopped growing and stopped keeping clear of the arc. A
  dial is the only screen where that bug is visible at a glance.

Both were bugs on every platform. The watch is what found them.

## What is missing

- **A crown that moves a control.** `(crown)` reaches the template and a value
  can be moved by hand, but an `an-slider` or an `an-stepper` does not take it
  from the system the way watchOS's do, where having focus is enough. Here the
  focus and the axis would have to be carried to them, and that is the
  primitive's job.
- **An app cannot tell it is on Wear.** `Device.info().platform` returns
  `"android"` on a watch: the Java side hard-codes it, and nothing anywhere
  produces `"wearos"`, even though `NativePlatform` declares it.
- **`AnHost.isWatch()` has no callers.** The accessor documented as the one the
  Activity consults is currently dead code.
- **Ambient mode.** A real watch drops to a black-and-white 1 Hz screen when the
  wrist goes down. Today the app simply closes, which is what Wear OS does with
  an app that does not declare it. There is no `AmbientModeSupport`, no
  `WearableActivity` and no `androidx.wear` dependency anywhere. It is another
  system surface with a lifecycle of its own.
- **Complications, tiles and watch faces.** They share nothing with this.
- **`an-text-input`.** It mounts and opens the system keyboard, but it has not
  been verified that Wear's input flow — dictation and scribble — returns the
  text where the host expects it.
- **Square watches.** They work — the round inset is applied only if the system
  says the screen is round — but they have not been tried.
- **A real watch.** Everything here was seen on the `an-wear` emulator, a Wear
  OS 5 image of 454 px at density 2. Left to see on hardware: the physical
  crown, video, which has no codecs there, and the battery, which on an OLED
  screen is half the design.

Accessibility on a watch is the Android accessibility contract unchanged, plus
what TalkBack on Wear does differently — see
[Accessibility on Android](/accessibility/android/).
