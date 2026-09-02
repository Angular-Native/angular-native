---
title: Accessibility on Apple
description: Where the six accessibility props land in UIKit, AppKit and SwiftUI, what has no equivalent on each, and how it is checked by reading the tree from outside the app.
---

Six props on every primitive say what a screen reader is told about a view:

```html
<an-view
  [accessibilityRole]="'button'"
  [accessibilityLabel]="'Play'"
  [accessibilityHint]="'Starts the track from the beginning'"
  [accessibilityValue]="'60 per cent'"
  [accessibilityState]="{ selected: true, disabled: true }"
  [accessible]="true"></an-view>
```

They are declared once, in `NativeVisual`, and they mean the same thing
everywhere. What is **not** the same anywhere is how they are written down: the
three Apple accessibility APIs are not one API with three sets of names, and
the differences are big enough that each host solves this its own way.

## Three APIs, three shapes

**UIKit** — the role is one bit of a `u64`. `accessibilityTraits` is a mask
where what a view *is* (button, header, image), what it *is like* right now
(not enabled, selected) and what it *does* (plays sound, turns pages) all share
one word. The contract splits role from state, so the host has to put them back
together, and it has to write the whole mask every time: setting one bit means
knowing the other sixty-three.

**AppKit** — the role is one string, and the states are not in it.
`setAccessibilityRole:` takes exactly one role, and `disabled`, `selected` and
`expanded` are separate properties of the `NSAccessibility` protocol. Nothing to
rebuild, and room for things UIKit cannot say.

**SwiftUI**, on the watch — neither, because there is no live view to call a
setter on. The tree is rebuilt from every snapshot, so the translation happens
in Rust while the snapshot is being built and the shell only attaches
`.accessibilityLabel()`, `.accessibilityAddTraits()` and the rest to the view it
is constructing.

## Role, platform by platform

| `accessibilityRole` | UIKit trait | AppKit role | SwiftUI trait |
|---|---|---|---|
| `button` | `.button` | `AXButton` | `.isButton` |
| `link` | `.link` | `AXLink` | `.isLink` |
| `header` | `.header` | `AXHeading` | `.isHeader` |
| `image` | `.image` | `AXImage` | `.isImage` |
| `text` | `.staticText` | `AXStaticText` | `.isStaticText` |
| `checkbox` | `.toggleButton` | `AXCheckBox` | `.isToggle` |
| `radio` | **none** | `AXRadioButton` | **none** |
| `switch` | `.toggleButton` | `AXCheckBox` + `AXSwitch` subrole | `.isToggle` |
| `slider` | `.adjustable` | `AXSlider` | **none** |
| `search` | `.searchField` | `AXTextField` + `AXSearchField` subrole | `.isSearchField` |
| `summary` | `.summaryElement` | **none** | `.isSummaryElement` |
| `none` | the mask cleared | `AXUnknown` | nothing added |

Three cells say **none**, and none of the three is rounded to the trait next
door:

- **`radio` has no UIKit trait.** There is nothing near it either. A radio
  button on iOS is announced by its value and by its position in its group —
  "two of five" — and that is a structure, not a trait. Nothing is applied and
  the host says so once, with the name of what was asked for.
- **`slider` has no SwiftUI trait.** Adjustable in SwiftUI is not a trait but an
  action, `accessibilityAdjustableAction`. Attaching one that did nothing would
  make the reader offer a gesture that goes nowhere.
- **`summary` has no AppKit role.** It is a VoiceOver-on-iOS idea: the element
  read on its own when you enter a screen, to sum it up. On a Mac there is no
  such moment — you do not "enter" a window — and there is neither a role nor a
  subrole like it.

Everything in that table is expressible on Android, which is a different shape
of platform rather than a better one: it has no role field at all, so most roles
go in as a widget class name and the three without a widget go in as spoken text
from a translated string. See [Accessibility on Android](/accessibility/android/).

Two more things the table hides. **UIKit does not tell a checkbox from a
switch**: it has one trait, "a button that turns on and off", and that describes
both — what separates them is the drawing, and the drawing is not read out.
SwiftUI does the same. And **AppKit's subroles are not decoration**: a search
field in AppKit *is* an `AXTextField`, and the only thing that tells it apart
from any other field is its subrole, so setting the role alone would leave it
indistinguishable from what it is not.

### `none` is a role, and the role it names is no role

`accessibilityRole="none"` is not the same as not writing the prop at all.
Dropping the binding hands the view back whatever it was — a system button goes
back to being announced as a button. `none` takes the role away, including the
one the control underneath was carrying: UIKit clears the mask, AppKit sets
`AXUnknown`. It is the same split the Android host makes, where `none` is the
`android.view.View` class, the one that means nothing.

The element stays in the tree either way, so a name on it is still read. It is
just announced as nothing in particular.

## State, platform by platform

| `accessibilityState` | UIKit | AppKit | SwiftUI |
|---|---|---|---|
| `disabled` | `.notEnabled` trait | `setAccessibilityEnabled(false)` | **none** |
| `selected` | `.selected` trait | `setAccessibilitySelected(true)` | `.isSelected` |
| `checked` | value `"1"` / `"0"` | value `0` / `1` / `2` | value `"1"` / `"0"` |
| `checked: 'mixed'` | **none** | value `2` | **none** |
| `expanded` | **none** | `setAccessibilityExpanded(true)` | **none** |
| `busy` | **none** | **none** | **none** |

`checked` is nobody's trait. Where it goes is the value, and it goes in with the
platform's **own convention** rather than a word of ours: a `UISwitch` publishes
`"1"` or `"0"` and VoiceOver turns that into "on" or "off" in whatever language
the device is set to. A string written by this project would come out in English
on a phone set to Japanese. AppKit uses numbers for the same thing, and that is
why the halfway state fits there and nowhere else — a macOS checkbox really does
publish `2`.

That value is only written over one nobody claimed. If the template set
`accessibilityValue`, the template's wins and `checked` does not touch it.

`busy` has no shape on any Apple platform. The nearest thing in AppKit is the
`AXBusyIndicator` role, which is *a spinner* — a different view, not a state of
this one — so setting it would turn a button into a spinner. Android does have
somewhere to put it, in spoken text, which is why the row above is the only one
where all three Apple platforms are empty and Android is not.

## What the system already got right

A `UIButton`, a `UISwitch` and an `NSButton` arrive with their name and their
role already set by the system: an `NSButton`'s accessibility label is its
title, an `NSSwitch` is already an `AXCheckBox` with the `AXSwitch` subrole.
Writing over that without looking makes things worse, and the worst case is
quiet: an empty label is not "no label", it is a name that overrides the good
one, and the control goes silent.

So the rule is that **only what the template really set gets overwritten**: an
empty string or a cleared binding removes our label rather than writing an empty
one.

On UIKit that is the whole story — clearing the label hands back the control's
own, and the traits a view carried are saved and restored the same way.

**On AppKit it is not**, and this is the sharpest edge in the whole area.
Overriding *anything* on an `NSView` — a label is enough — makes AppKit stop
working that view's accessibility out for itself and start serving the
overrides. The role, which nobody overrode, then comes back `AXUnknown`: a
button given nothing but a better name stops being announced as a button.

It cannot be fixed by saving the role and putting it back, because the role
cannot be read. In-process, `NSButton.accessibilityRole()` answers `AXUnknown`
while a real assistive client is told `AXButton` — AppKit computes it in the
cell, on demand, for the client. So what the host writes back is not a saved
value but the role the primitive genuinely has: an `an-button` mounts an
`NSButton` and an `NSButton` is an `AXButton`, which is the same fact the
platform inventory already states as a class name.

When a role **is** given it replaces rather than adds. A template writing
`accessibilityRole="link"` on an `an-button` is saying this reads as a link, not
as "link, button".

## A role or a name on its own is not enough

Neither `UIView` nor `NSView` is an accessibility element by default. A plain
`an-view` given `accessibilityRole="slider"` and nothing else has its role set
correctly, is never published to a reader, and produces no error anywhere. The
same is true of a view given only a label — which is the most common use of the
whole contract, and was the quietest failure in it.

So on both hosts a role the platform can honour makes the view an element, and
so does a name. `accessible` overrules both, because that is the prop that
exists to say exactly this:

| what the template said | is it a stop? |
|---|---|
| `accessible="true"` | yes, and what is inside stops being separate stops |
| `accessible="false"` | no, and neither is anything inside it |
| a role the platform has, or a label | yes |
| nothing | whatever the view already was |

`accessible="false"` has to reach the children too. Dropping the element on its
own hides nothing: UIKit needs `accessibilityElementsHidden`, and AppKit needs
the children list emptied, or a decorative box with a label inside is still a
stop.

## How this is checked

**Setting a property does not prove a reader can read it.** Every failure above
— the lost role, the view that is not an element, the empty label over a good
one — passes any check that reads back the property it just wrote, and none of
them shows up in a log or in a screenshot. Every one of them was found by
reading the tree from outside the app, and none of them would have been found
any other way.

```bash
./scripts/check-accessibility.sh            # in check-all.sh
./scripts/check-accessibility-simulator.sh  # needs a simulator; run on purpose
```

Both launch the app and read its tree from a **separate process** —
`scripts/ax-dump.swift`, which never links against it and has nothing but a
pid — through `AXUIElementCopyAttributeValue`, the same door VoiceOver and
Accessibility Inspector use.

On **macOS** the app runs on the machine itself. On **iOS** it works because the
Simulator bridges the guest app's tree into the host's accessibility API, which
is how Accessibility Inspector inspects a simulator; the walker asks
`Simulator.app` instead of the app. What comes back is UIKit's mask already
translated into AX attributes by the platform:

```text
AXGroup[iOSContentGroup]
  AXHeading label=" Accessibility "
  AXButton label="Play" help="Starts the track from the beginning"
  AXButton label="Untouched button"
  AXButton label="Save the changes you made"
  AXGenericElement label="Stripped of its role"
  AXCheckBox[AXSwitch] label="Night mode" value="1"
  AXSlider label="Volume" value="60 per cent"
  AXButton label="Chosen and switched off" enabled=false selected=true
```

Half the assertions are about what must **not** be in there, because a check
that only looks for what it expects cannot catch the opposite failure: the
decorative row is absent, the grouped row's icon and text are not two extra
stops of their own, and nothing is published as an `AXRadioButton` on iOS, where
no such trait exists.

Both need the Accessibility permission, which is granted by hand per app in
System Settings and cannot be granted from a script. Without it they say so and
skip: a machine without the grant has broken nothing, and a check that failed
for that reason would be a check nobody believes.

### What is not verified this way

**tvOS and visionOS** mount the same UIKit host as iOS, so the mapping is the
same code — but their simulators were not walked, so that is inference, not
evidence.

**watchOS** has no outside route at all. What is checked there is what can be:
that the six props travel with the right names, that the role vocabulary in the
core is exactly the one the contract declares, that every trait name Rust emits
exists in the Swift table that turns it into an `AccessibilityTraits`, and that
the shell type-checks. None of that is the same as knowing a reader sees it.

**VoiceOver itself was never running.** It can be switched on in the simulator's
defaults, and the app then runs normally — but the VoiceOver process does not
start, nothing is spoken, and no cursor appears. Reading the tree is as close as
this gets, and it is worth being clear that "the tree an assistive client would
see" and "a screen reader said the right words" are two different claims.

## The example

`examples/a11y-apple` is the screen both checks read. Every row in it is a case
one of the hosts had to decide something about, including the ones no Apple
platform can express, so that "it is said out loud" is something a check can
read in the log rather than something a comment claims.

```bash
cargo an macos examples/a11y-apple     # on this machine
cargo an ios examples/a11y-apple       # in the simulator
```
