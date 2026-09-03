---
title: Components
description: The 25 primitives, what each one becomes on every platform, and where it is not available.
sidebar:
  order: 1
---

Every primitive maps to a control the system already has. Where a platform has
no equivalent, the node is **not created**: the layout leaves the gap it had
measured and the host says why, once, in the log. Nothing is drawn by hand to
fill in for a missing control.

## Support matrix

| Primitive | iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|---|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| `an-view` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `an-text` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `an-image` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `an-icon` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `an-scroll-view` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `an-button` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `an-text-input` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `an-textarea` | ✅ | ✅ | ✅ | ✅ | ✅ | — | ✅ |
| `an-switch` | ✅ | ✅ | ✅ | — | ✅ | ✅ | ✅ |
| `an-slider` | ✅ | ✅ | ✅ | — | ✅ | ✅ | ✅ |
| `an-stepper` | ✅ | ✅ | ✅ | — | ✅ | ✅ | ✅ |
| `an-progress-bar` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `an-activity-indicator` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `an-select` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — |
| `an-date-picker` | ✅ | ✅ | ✅ | — | ✅ | ✅ | — |
| `an-segmented-control` | ✅ | ✅ | ✅ | ✅ | ✅ | — | — |
| `an-search-bar` | ✅ | ✅ | ✅ | ✅ | ✅ | — | — |
| `an-tab-bar` | ✅ | ✅ | ✅ | ✅ | ✅ | — | — |
| `an-navigation-bar` | ✅ | ✅ | title bar | ✅ | ✅ | — | — |
| `an-stack-view` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `an-modal` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `an-alert` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `an-web-view` | ✅ | ✅ | ✅ | — | ✅ | — | — |
| `an-map-view` | ✅ | tiles | ✅ | ✅ | ✅ | — | ✅ |
| `an-video-view` | ✅ | ✅ | ✅ | ✅ | ✅ | — | ✅ |
| | **25** | **25** | **25** | **20** | **25** | **17** | **18** |

`an-safe-area` is not in the table because it is not a primitive: it is a
component built on top of `an-view` that turns the insets the host reports into
padding. It mounts everywhere, but on watchOS the insets are always zero — the
app owns the whole screen and the system reserves no queryable margin — so there
it does nothing. On Wear OS it is how the round screen's margin arrives.

Two entries are neither a yes nor a no, and both are explained where they
happen: on macOS `an-navigation-bar` puts its title in the **window's title
bar**, because a Mac already has one and drawing a second inside the content
would be painting two; on Android `an-map-view` draws **OpenStreetMap tiles**
on a canvas, because the platform ships no map — Google's lives in Play
Services behind an API key.

## What each one becomes

| Primitive | iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|---|
| `an-view` | `UIView` | `ViewGroup` | `NSView` | `ZStack` |
| `an-text` | `UILabel` | `TextView` | `NSTextField` | `Text` |
| `an-image` | `UIImageView` | `ImageView` | `NSImageView` | `Image` |
| `an-icon` | SF Symbol | Material Symbols | SF Symbol | SF Symbol |
| `an-scroll-view` | `UIScrollView` | `ScrollView` | `NSScrollView` | `ScrollView` |
| `an-button` | `UIButton` | `MaterialButton` | `NSButton` | `Button` |
| `an-text-input` | `UITextField` | `EditText` | `NSTextField` | `TextField` |
| `an-textarea` | `UITextView` | `EditText` | `NSTextView` | — |
| `an-switch` | `UISwitch` | `MaterialSwitch` | `NSSwitch` | `Toggle` |
| `an-slider` | `UISlider` | `Slider` | `NSSlider` | `Slider` |
| `an-stepper` | `UIStepper` | two icon buttons | `NSStepper` | `Stepper` |
| `an-progress-bar` | `UIProgressView` | `LinearProgressIndicator` | `NSProgressIndicator` | `ProgressView` |
| `an-select` | `UIButton` + `UIMenu` | `Spinner` | `NSPopUpButton` | `Picker` |
| `an-date-picker` | `UIDatePicker` | system dialog | `NSDatePicker` | `DatePicker` |
| `an-tab-bar` | `UITabBarController` | `BottomNavigationView` | `NSSegmentedControl` | — |
| `an-alert` | `UIAlertController` | `MaterialAlertDialog` | `NSAlert` | `.alert` |
| `an-web-view` | `WKWebView` | `android.webkit.WebView` | `WKWebView` | — |
| `an-video-view` | `AVPlayerViewController` | `VideoView` | `AVPlayerView` | — |

## Props every primitive takes

These live on the base, so they work on all 25.

| Prop | Type | What it does |
|---|---|---|
| `backgroundColor` | `string` | `#rgb`, `#rrggbbaa`, `rgb()`, `rgba()` and a handful of names. |
| `borderRadius` | `number` | Also per corner: `borderTopLeftRadius` and its three siblings. |
| `borderWidth`, `borderColor` | `number`, `string` | |
| `opacity` | `number` | |
| `translateX`, `translateY`, `scale`, `scaleX`, `scaleY`, `rotate` | `number` | Deliberately outside layout: a moved view still occupies its old place, so there is nothing to recompute and it can follow a finger. |
| `animate` | `number` | Milliseconds. From then on **changes** to that view are animated by the platform, on its own drawing thread. `animateDelay` and `animateEasing` go with it. |
| `cursor` | `NativeCursor` | macOS only — a finger has no shape. |
| `testID` | `string` | Ends up in `accessibilityIdentifier`. |

### Accessibility

| Prop | Type | What it does |
|---|---|---|
| `accessibilityLabel` | `string` | The name a screen reader announces. System controls bring their own; set this when theirs does not say the right thing. |
| `accessibilityHint` | `string` | What activating it does, read after the name. |
| `accessibilityRole` | `NativeRole` | `button`, `link`, `header`, `image`, `text`, `checkbox`, `radio`, `switch`, `slider`, `search`, `summary`, `none`. |
| `accessibilityValue` | `string` | What it reads right now: "35%", "three of seven". |
| `accessibilityState` | `NativeAccessibilityState` | `disabled`, `selected`, `checked` (`true`, `false` or `'mixed'`), `expanded`, `busy`. |
| `accessible` | `boolean` | Whether this is **one** element or a container to navigate into. `false` hides it and its contents — what decoration needs. |

## Events every primitive emits

| Event | Payload | Notes |
|---|---|---|
| `(press)`, `(doublePress)`, `(longPress)` | `{x, y}` | On tvOS these arrive from the remote's centre button, not a touch. |
| `(pan)` | translation, velocity, state | Translation is from where the finger started, not from the previous event. |
| `(pinch)`, `(rotation)` | scale / radians | Not on tvOS: the remote surface is single-touch. |
| `(swipeLeft/Right/Up/Down)` | `{x, y}` | Each direction attaches its own recogniser, so listening for one costs nothing for the other three. |
| `(hover)` | `{hovered, x, y}` | macOS only. |
| `(focus)`, `(blur)` | `{value?}` | The whole platform on a TV; the keyboard on a phone. |
| `(crown)`, `(crownIdle)` | `{delta, offset, velocity}` | The digital crown, on both watches. Delivered to whichever view holds focus. |
| `(layout)` | `{x, y, width, height}` | Emitted by the core, not by a platform — it is the one computing the frame, so it is free everywhere. |
| `(safeArea)` | insets | Changes on rotation and when the keyboard opens. |

An event a platform cannot deliver **warns when you subscribe to it**, rather
than staying silent. That is the difference between a gap and a bug.
