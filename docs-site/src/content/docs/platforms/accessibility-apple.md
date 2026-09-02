---
title: Accessibility on Apple
description: How the six accessibility props reach UIKit, AppKit and SwiftUI, what has no equivalent on each, and how it is checked from outside the app.
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
| `none` | the traits the view already had | the role AppKit gave it | nothing added |

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

Two more things the table hides. **UIKit does not tell a checkbox from a
switch**: it has one trait, "a button that turns on and off", and that describes
both — what separates them is the drawing, and the drawing is not read out.
SwiftUI does the same. And **AppKit's subroles are not decoration**: a search
field in AppKit *is* an `AXTextField`, and the only thing that tells it apart
from any other field is its subrole, so setting the role alone would leave it
indistinguishable from what it is not.

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
this one — so setting it would turn a button into a spinner.

## What the system already got right

A `UIButton`, a `UISwitch` and an `NSButton` arrive with their name and their
role already set by the system: an `NSButton`'s accessibility label is its
title, an `NSSwitch` is already an `AXCheckBox` with the `AXSwitch` subrole.
Writing over that without looking makes things worse, and the worst case is
quiet: an empty label is not "no label", it is a name that overrides the good
one, and the control goes silent.

So the rule is that **only what the template really set gets overwritten**:

- an empty string or a cleared binding **removes** our label instead of writing
  an empty one, and both UIKit and AppKit then hand back the system's;
- the traits (UIKit) and the role (AppKit) a view carried are saved the first
  time they have to be overwritten, and come back when the template says
  `'none'` or drops the prop;
- a host writes back only what the template asked about. Rewriting the role a
  view already had is not a no-op — it *pins* it, replacing AppKit's own
  computation with a fixed answer — and a view whose role gets pinned to nothing
  drops out of the accessibility tree and takes its window with it.

When a role **is** given it replaces rather than adds. A template writing
`accessibilityRole="link"` on an `an-button` is saying this reads as a link, not
as "link, button".

## A role on its own is not enough

This is the part that only shows up when the tree is read from outside, and it
is the reason the check below exists in the form it does.

Neither `UIView` nor `NSView` is an accessibility element by default. A plain
`an-view` given `accessibilityRole="slider"` and nothing else has its role set
correctly, is never published to a reader, and produces no error anywhere: the
walk from outside comes back with nothing where the slider should be.

So on both hosts a role the platform can honour also makes the view an element.
`accessible` overrules it in either direction, because that is the prop that
exists to say exactly this:

| what the template said | is it a stop? |
|---|---|
| `accessible="true"` | yes, and what is inside stops being separate stops |
| `accessible="false"` | no, and neither is anything inside it |
| a role the platform has | yes |
| nothing | whatever the view already was |

`accessible="false"` has to reach the children too. Dropping the element on its
own hides nothing: UIKit needs `accessibilityElementsHidden`, and AppKit needs
the children list emptied, or a decorative box with a label inside is still a
stop.

## How this is checked

**Setting a property does not prove a reader can read it.** Every failure above
— the pinned role, the view that is not an element, the empty label over a good
one — passes any check that reads back the property it just wrote, and none of
them shows up in a log or in a screenshot.

So the check that carries the weight is an outside one, and it exists on exactly
one Apple platform:

```bash
./scripts/check-accessibility.sh
```

On **macOS** it launches the app and a **separate process** —
`scripts/ax-dump.swift`, which never links against it and has nothing but a
pid — reads the tree through `AXUIElementCopyAttributeValue`, the same door
VoiceOver and Accessibility Inspector use. What comes back is what an assistive
client would get:

```text
AXWindow[AXStandardWindow] title="angular-native"
  AXHeading value=" Accessibility "
  AXButton label="Play" help="Starts the track from the beginning"
  AXButton title="Untouched button"
  AXButton label="Save the changes you made" title="Save"
  AXCheckBox[AXSwitch] label="Night mode" value="1"
  AXSlider label="Volume" value="60 per cent"
  AXButton label="Chosen and switched off" enabled=false selected=true
  AXRadioButton label="No trait in UIKit"
```

Half the assertions are about what must **not** be in there, because a check
that only looks for what it expects cannot catch the opposite failure: the
decorative row is absent, the grouped row's icon and text are not two extra
stops of their own, and the role AppKit has no answer for is left out rather
than faked.

It needs the Accessibility permission, which is granted by hand per app in
System Settings and cannot be granted from a script. Without it the check says
so and skips: a machine without the grant has broken nothing, and a check that
failed for that reason would be a check nobody believes.

**On iOS, tvOS, visionOS and watchOS there is no outside route**, and that is
worth saying plainly rather than dressing up. The simulator does not publish the
guest app's accessibility tree to the host's accessibility API, and there is no
`simctl` verb that dumps it; the alternative — having the app read back its own
properties — would prove that a setter works and nothing about whether a reader
sees the result, which is the exact mistake this section is about. What is
checked on those four is what can be: that the six props travel with the right
names into every host, that the role vocabulary in the core is exactly the one
the contract declares, and — for the watch, the only host split across two
languages — that every trait name Rust emits exists in the Swift table that
turns it into an `AccessibilityTraits`.

## The example

`examples/a11y` is the screen the check reads. Every row in it is a case one of
the hosts had to decide something about, including the three that no Apple
platform can express, so that "it is said out loud" is something the check can
read in the log rather than something a comment claims.

```bash
cargo an macos examples/a11y     # on this machine
cargo an ios examples/a11y       # in the simulator
```
