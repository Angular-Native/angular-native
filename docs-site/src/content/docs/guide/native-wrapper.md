---
title: Props and the native wrapper
description: How much of the real control a template can reach, how a prop travels, and what was deliberately left out of each one.
sidebar:
  order: 5
---

Every primitive wraps a real native control. The question this page answers is
how much of that control a template can touch, and what is still out of reach.

## The three rules

1. **What exists on both platforms is called the same thing** and is an ordinary
   input: `[variant]`, `[placeholder]`, `[enabled]`. The name comes from the API,
   not from whichever platform spells it more prettily.
2. **What exists on only one goes in that one's object**: `[ios]="{…}"` and
   `[android]="{…}"`. A common name is never invented for something only one
   platform has — that would force the other to imitate it, and an imitation
   shows.
3. **Nothing is lost in silence.** A key the host does not recognise warns once
   in the console, exactly as an unknown style does.

The first two are checked by the template compiler: props are declared inputs
and the per-platform objects are closed types, so `[ios]="{ subtitel: 'x' }"`
does not compile. The third is for what the compiler cannot see — an object
assembled at runtime, a spread, an `any` that slipped through.

That a prop really reaches the other end is checked by `scripts/check-wrapper.sh`,
which compares the directives' prop list against what the iOS crate, the Android
shell and the macOS crate actually read.

## How a prop travels

Props are **signal inputs** — `input()` — not `@Input() set`. A signal input has
no moment at which it "is assigned": it is read, and whoever reads it decides
when.

Here it is read by an effect per group of props, not an effect per prop. An
`<an-text>` declares ten of its own and hardly any template uses more than
three, so an effect per prop would leave every `<an-text>` in a five-thousand-row
list carrying ten reactive nodes nobody is going to wake. Reading ten signals
when one changes is cheaper. In practice a primitive installs one effect per
class in its inheritance chain that declares props, plus one per platform object
— an `<an-view>` installs one, an `<an-text>` three, an `<an-button>` five.

Out of the effect comes only what changed, and on the first pass the nulls stay
quiet too: that is what an input nobody set is worth, and sending it would be
asking the host to erase something it never wrote.

One extra effect earns its keep in one place: an `<an-icon>` with no `[size]` no
longer ends up with no size. A `set` nobody binds never runs — which is why the
constructor used to have to call it by hand — while a signal is read even when
nobody writes it.

### Names that change on the wire

Four inputs do not travel under their own name, and it is worth knowing when you
are reading a log or a host:

| Input | Prop on the wire | Why |
|---|---|---|
| `an-stepper` `[step]` | `stepValue` | |
| `an-icon` `[size]`, `[weight]` | `iconSize`, `iconWeight` | `size` and `weight` are too generic to claim. |
| `(rotation)` | the event `rotate` | `[rotate]` is already the transform input. |

`[items]`, `[icons]`, `[buttons]` and `[accessibilityState]` travel as JSON
strings, because the wire's value type has no array and no object. A null
`accessibilityState` stays null rather than becoming `"{}"`.

## How a platform prop travels

`[ios]="{ subtitle: 'x' }"` does not send an object: the directive takes it
apart and sends `ios:subtitle`. The prefix does two things. First, the Android
host can discard it without knowing what it is. Second, the name stays
greppable: the check demands that `"ios:subtitle"` appear in the iOS host and
**not** in the Android one — appearing in both means it was a common prop
wearing a prefix.

Two details:

- **A key that disappears is un-set.** The directive remembers which keys it
  sent last time, and one that is gone from the object is re-sent as null, so
  the control goes back to its factory value rather than keeping the last thing
  you gave it.
- **An unknown key warns once per key**, the same way an unknown style does.

The per-platform types are `type` aliases rather than interfaces, deliberately:
an interface has no index signature, so it could not be passed to the code that
takes the object apart.

## What every primitive already has

Twenty-five inputs and seventeen outputs live on the base that all twenty-five
primitives extend — background, border, opacity, the transforms, `animate`,
`cursor`, `testID`, the six accessibility props, and every gesture. They are
listed in [Components](/reference/components/).

Eight primitives extend a second base that adds exactly one input, `[enabled]`:
`an-button`, `an-switch`, `an-slider`, `an-segmented-control`, `an-stepper`,
`an-search-bar`, `an-select` and `an-date-picker`. `an-activity-indicator` and
`an-progress-bar` do not get it because you do not touch them, and the text
fields do not because they already have `[editable]`, which is the same idea
under the name a field uses.

Everything below is what each primitive adds **on top of** that.

## The inventory

### `an-button` — `UIButton` · `MaterialButton` · `NSButton`

| Prop | iOS | Android |
|---|---|---|
| `title` | `setTitle:forState:` | `setText` |
| `color` | `tintColor` | tint per variant |
| `variant` `text\|filled\|tonal\|outlined` | `UIButtonConfiguration` | Material style |
| `icon` | `configuration.image` (SF Symbol) | `setIcon` (Material Symbol) |
| `iconPosition` `leading\|trailing` | `imagePlacement` | `setIconGravity` |
| `fontSize`, `fontWeight` | `titleTextAttributesTransformer` | `setTextSize`, `setTypeface` |
| `[ios].subtitle` | `configuration.subtitle` | — |
| `[android].rippleColor` | — | `setRippleColor` |
| `[android].allCaps` | — | `setAllCaps` |

A tap arrives through the inherited `(press)`: the button has no output of its
own.

**Left out:** `elevated` as a variant — Material has it and UIKit has nothing
equivalent, so it would be a variant that does something on half a platform;
ask for it through `[android]`. `contentInsets` — the space inside a button is
the system's call, and touching it is exactly what stops a button looking like
the platform's. `cornerRadius` — unnecessary: `[borderRadius]` is a prop of any
view and a button is a view.

:::caution[A subtitle needs a height]
A two-line button does not fit in a button's natural height. Each control's size
is asked of the platform once at startup, from a sample control — creating a
`UIButton` needs the main thread and the measurer lives on the engine's — so
that sample cannot know this one will carry a subtitle. Give it a height in the
template, or the title gets clipped and the subtitle is left on its own.
:::

### `an-text-input` — `UITextField` · `EditText` · `NSTextField`

| Prop | iOS | Android |
|---|---|---|
| `value`, `placeholder`, `editable` | | |
| `secureTextEntry` | `isSecureTextEntry` | password `InputType` |
| `color`, `fontSize`, `fontWeight`, `fontFamily` | `textColor`, `font` | `setTextColor`, `setTextSize`, `setTypeface` |
| `keyboardType` | `keyboardType` | `InputType` |
| `returnKeyType` | `returnKeyType` | `imeOptions` |
| `autoCapitalize` | `autocapitalizationType` | `InputType` flags |
| `autoCorrect` | `autocorrectionType` | `NO_SUGGESTIONS` |
| `placeholderColor` | `attributedPlaceholder` | `setHintTextColor` |
| `textAlign` | `textAlignment` | `gravity` |
| `[ios].clearButtonMode` | `clearButtonMode` | — |
| `[ios].borderStyle` | `borderStyle` | — |
| `[android].selectAllOnFocus` | — | `setSelectAllOnFocus` |
| `[android].cursorVisible` | — | `setCursorVisible` |

Outputs `(valueChange)` and `(submit)`. `(focus)` and `(blur)` used to live here
and now live on the base, so every primitive has them.

**Left out:** `maxLength`. Android does it with a one-line `InputFilter`; iOS
has nothing for it and you have to intercept the delegate's
`shouldChangeCharactersInRange`, which means putting a delegate of our own on a
control that today carries only actions. It can be done; a `maxLength` that only
works on Android is worse than not having one.

### `an-text` — `UILabel` · `TextView` · `NSTextField`

| Prop | iOS | Android |
|---|---|---|
| `color`, `fontSize`, `fontWeight`, `fontStyle`, `fontFamily` | | |
| `textAlign`, `numberOfLines` | | |
| `lineHeight` | `NSParagraphStyle` | `setLineSpacing` |
| `letterSpacing` | `NSAttributedString` `kern` | `setLetterSpacing` |
| `textDecoration` `none\|underline\|lineThrough` | text attributes | `Paint` flags |
| `[android].selectable` | — | `setTextIsSelectable` |

**Left out:** `adjustsFontSizeToFitWidth`. `UILabel` has it and `TextView` has
auto-sizing since API 26, so it would be a legitimate common prop. What keeps it
out is that the core measures text itself for layout and does not know how to
shrink, so the box would still be the big size. Doing it properly means touching
measurement, not the host.

### `an-switch` — `UISwitch` · `MaterialSwitch` · `NSSwitch`

| Prop | iOS | Android |
|---|---|---|
| `on`, `color` | `isOn`, `onTintColor` | `isChecked`, track tint |
| `thumbColor` | `thumbTintColor` | `setThumbTintList` |
| `[android].trackColor` | — | the off track |

**Left out:** the off-track colour on iOS. `UISwitch` does not expose it. What
circulates is giving a system control a `backgroundColor` and a corner radius of
16 so its background shows through behind it, which breaks the day Apple changes
the control's height. That is exactly the ugly hack this project does not do: on
iOS it keeps the system colour.

### `an-slider` — `UISlider` · Material `Slider` · `NSSlider`

| Prop | iOS | Android |
|---|---|---|
| `value`, `minimumValue` (0), `maximumValue` (1), `color` | | |
| `minimumTrackColor` | `minimumTrackTintColor` | `setTrackActiveTintList` |
| `maximumTrackColor` | `maximumTrackTintColor` | `setTrackInactiveTintList` |
| `thumbColor` | `thumbTintColor` | `thumbTintList` |
| `[ios].continuous` | `isContinuous` | — |
| `[android].stepSize` | — | `setStepSize` |

Note the maximum defaults to **1**, not 100. `an-stepper` is the one that goes
to 100.

**Left out:** `stepSize` as a common prop. `UISlider` is continuous and has no
steps; rounding the value in the host is possible, but then the finger goes one
way and the value another, and the control stops giving the tactile response
Android's does, which really does snap. Promising "steps" and delivering two
different behaviours is worse than saying only Android has them.

### `an-scroll-view` — `UIScrollView` · `AnScrollView` · `NSScrollView`

| Prop | iOS | Android |
|---|---|---|
| `horizontal` | `alwaysBounce…`, one axis of the `contentSize` | an inner `HorizontalScrollView` |
| `showsScrollIndicator` | `showsVertical…` | both scrollbars |
| `refreshing` | `UIRefreshControl` | an arc of our own |
| `bounces` | `bounces` | `overScrollMode` |
| `scrollEnabled` | `isScrollEnabled` | swallows the gesture |
| `[ios].pagingEnabled` | `isPagingEnabled` | — |
| `[ios].keyboardDismissMode` | `keyboardDismissMode` | — |

**Left out:** both axes at once. The core clamps `contentSize` to the view's own
frame across the axis it is not scrolling on, so a scroll view offers one
direction and only one. Android is the reason it stays that way: `ScrollView`
and `HorizontalScrollView` are two classes, and the two nested inside each other
fight over every diagonal drag.

### `an-tab-bar` — `UITabBarController` · `AnTabBar` · `NSSegmentedControl`

| Prop | iOS | Android |
|---|---|---|
| `items`, `icons`, `selectedIndex`, `color` | | |
| `unselectedColor` | `unselectedItemTintColor` | inactive colour |
| `[ios].translucent` | `isTranslucent` | — |

`items` and `icons` travel as JSON strings.

**Left out:** badges. On iOS it is `UITabBarItem.badgeValue` and comes free; on
Android the bar is ours — the platform ships none — so the bubble would have to
be drawn by hand, and a hand-drawn bubble next to a system one do not look
alike. Noted as work on the Android tab bar, not on the wrapper.

### `an-select` — `UIButton` + `UIMenu` · `Spinner` · `NSPopUpButton`

Just `items` and `selectedIndex`, plus the inherited `enabled`.

**Left out:** `mode` (`dropdown` or `dialog`). Android's `Spinner` decides in
its constructor and cannot be changed afterwards, and iOS does not have the two
shapes at all — a `UIMenu` is always a menu. Supporting it would mean destroying
and rebuilding the view mid-flight, which is precisely what the tree avoids.

### The rest

`an-view` adds nothing at all — it is the pure demonstration of what the base
gives you.

| Primitive | Its own props |
|---|---|
| `an-stack-view` | `transition` `push\|pop\|none`; output `(back)` |
| `an-image` | `source`, `resizeMode`, `intrinsicWidth`, `intrinsicHeight`; output `(load)` |
| `an-icon` | `name`, `size` (24), `weight`, `color` |
| `an-textarea` | `value`, `editable`, `color`; output `(change)` |
| `an-segmented-control` | `items`, `selectedIndex`, `color`; output `(change)` |
| `an-stepper` | `value`, `minimumValue`, `maximumValue` (100), `step`; output `(change)` |
| `an-search-bar` | `value`, `placeholder`; outputs `(input)`, `(submit)` |
| `an-date-picker` | `value` (a `Date` or ms), `mode` `date\|time\|dateAndTime`; output `(change)` |
| `an-navigation-bar` | `title`, `showsBack`, `backTitle`; output `(back)` |
| `an-progress-bar` | `progress`, `color` |
| `an-activity-indicator` | `animating`, `color` |
| `an-modal` | `visible`, `presentation` `fullScreen\|sheet`, `[ios].detents`; output `(dismiss)` |
| `an-alert` | `visible`, `title`, `message`, `buttons`, `sheet`; output `(select)` |
| `an-web-view` | `url`, `html` |
| `an-map-view` | `latitude`, `longitude`, `zoom` (12), `showsUser` |
| `an-video-view` | `url`, `playing`, `muted` |

`an-safe-area` is not in this list because it is not a primitive: it is a
component built on top of `an-view` that turns the insets the host reports into
padding. See [Components](/reference/components/).

## The check that keeps this honest

`scripts/check-wrapper.sh` compares the props the directives declare against
what the hosts read, and it fails the build on a mismatch. It reads the **whole**
iOS crate and the whole macOS crate, not one file each — the Apple accessibility
props live in their own module, and a check that only looked at the main host
file would report all six as unreachable.

It maintains three lists, and all three are the point:

- **Core-only.** `intrinsicWidth` and `intrinsicHeight`. Layout consumes them to
  reserve the space for an image that has not loaded yet, and they have nothing
  to say to any host.
- **Pointer-only.** `cursor`. The shape of a pointer only means something where
  there is a pointer, and a finger has no shape. Demanding that iOS and Android
  read it would be demanding something they cannot do, and putting it on a
  to-do list would imply they will one day. So the same demand is made of the
  **desktop** host instead, just as hard. Its matching output, `(hover)`, is not
  here because outputs are not props: they travel by a different path, and there
  each host says what it cannot deliver. See [macOS](/platforms/macos/).
- **Pending.** Props that ought to arrive and do not. **It is currently empty**,
  and the list can only shrink: a prop still on it that now reaches both hosts
  is also a failure, so nobody can leave a fixed leak marked as broken.

There is a structural check too: the number of `[ios]`/`[android]` inputs
declared has to equal the number of times the take-it-apart helper is called, so
nobody can smuggle a platform object out with a plain assignment and lose an
unknown key in silence.

That third list being empty is the current answer to "does this prop actually do
anything": **there are no known leaks**. It got there by finding eight — a
password rendered in plain text on Android, a spinner that never stopped, a
scrollbar that could not be hidden, a bounce that could not be turned off, two
font props that did nothing, and `lineHeight` and `letterSpacing`, which were
the worst of them: the core *measured* with them and the host drew without them,
so layout reserved room for a text with more leading than the one that got
painted.
