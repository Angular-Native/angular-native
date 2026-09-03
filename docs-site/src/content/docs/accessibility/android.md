---
title: Accessibility on Android
description: The six accessibility props of the contract, and where each one lands in AccessibilityNodeInfo.
sidebar:
  order: 2
---

A template writes six props and a screen reader announces something. Between
the two there is a translation, and on Android it is a longer one than it looks:
only two of the six are properties of a view. The rest do not exist as fields.

```html
<an-view
  [accessibilityRole]="'button'"
  [accessibilityLabel]="'Save the draft'"
  [accessibilityHint]="'saves it without leaving the screen'"
  (press)="save()">
  <an-text>Save</an-text>
</an-view>
```

That is an `AnViewGroup` — a bare `ViewGroup` — and TalkBack announces it as
"Save the draft, button, double tap to save it without leaving the screen".
Nothing about that is a property you can set on the view.

## The two halves

**What is a property of the view** is the name and whether this counts as one
element:

| Prop | Android |
|---|---|
| `accessibilityLabel` | `View.setContentDescription` |
| `accessible` | `View.setImportantForAccessibility` and `ViewCompat.setScreenReaderFocusable` |

**Everything else** — role, value and state — lives in the
`AccessibilityNodeInfo` a view fills in each time an accessibility service asks
about it. The only way in is to intercept that moment, which is what an
`AccessibilityDelegate` is for:

```java
@Override
public void onInitializeAccessibilityNodeInfo(View host, AccessibilityNodeInfoCompat info) {
    super.onInitializeAccessibilityNodeInfo(host, info);   // what the widget says
    decorate(host, info, state);                           // what the template said
}
```

The delegate is installed on the first node that receives a prop needing it,
and on no other. An app that uses none of this pays for nothing.

## The role

Android has no role field. What a screen reader uses to say "button" is the
class name of the node, which by default is the real view's. Seven of the twelve
roles have a widget whose name Android already knows how to announce, in the
system's language and with whatever word that version of TalkBack uses; three
do not, and go through `setRoleDescription`, which is text a reader speaks
verbatim — so it comes from `res/values/strings.xml` and is translated, not from
a constant in the code.

| `NativeRole` | `AccessibilityNodeInfo` |
|---|---|
| `button` | `setClassName("android.widget.Button")` |
| `image` | `setClassName("android.widget.ImageView")` |
| `text` | `setClassName("android.widget.TextView")` |
| `checkbox` | `setClassName("android.widget.CheckBox")` + `setCheckable(true)` |
| `radio` | `setClassName("android.widget.RadioButton")` + `setCheckable(true)` |
| `switch` | `setClassName("android.widget.Switch")` + `setCheckable(true)` |
| `slider` | `setClassName("android.widget.SeekBar")` |
| `header` | `setHeading(true)` — a flag, not a class; it is what TalkBack navigates headings by |
| `link` | `setRoleDescription`, from a translated string |
| `search` | `setRoleDescription`, from a translated string |
| `summary` | `setRoleDescription`, from a translated string |
| `none` | `setClassName("android.view.View")` — the class that means nothing, and the only way to take away a role the widget underneath was carrying |

A role the host does not know is an error in the log, not a silent no-op. It
should be unreachable — Angular's compiler rejects anything outside
`NativeRole` — so getting there means the contract and the host have drifted,
and `check-a11y.sh` is what stops that happening.

## The state

| `NativeAccessibilityState` | `AccessibilityNodeInfo` |
|---|---|
| `disabled` | `setEnabled(!disabled)` |
| `selected` | `setSelected` |
| `checked: true \| false` | `setCheckable(true)` + `setChecked` |
| `checked: 'mixed'` | `setCheckable(true)` + `setChecked(false)` + "partially checked" in `setStateDescription` |
| `expanded` | `addAction(ACTION_EXPAND)` or `ACTION_COLLAPSE` |
| `busy` | "busy" in `setStateDescription` |

Three things are worth knowing.

**`disabled` only touches the node.** Turning the real view off is what
`[enabled]` does, and it also stops it responding to touch. If
`accessibilityState` did that too, describing a state would change behaviour
without anyone having asked for it.

**`mixed` has nowhere to go.** `AccessibilityNodeInfo.setChecked` is a boolean.
The third state is a checkable node that is not checked, plus a
`stateDescription` that says so in words — the same thing Jetpack Compose does
with `ToggleableState.Indeterminate`. It is not dropped, and it is not faked as
`true`.

**`expanded` is an action, not a field.** On Android, expanding something is
`ACTION_EXPAND`; a reader announces "double tap to expand". So it is only set on
a node that responds to touch. On one that does not, the host logs an error and
sets nothing, because announcing an action that cannot happen is worse than
announcing nothing.

## The value

`accessibilityValue` goes to `setStateDescription`, the slot Android 11 opened
for exactly this: what a control is worth right now, announced after the name
and the role.

Three things can want that one slot — the value, the "partially checked" of
`mixed`, and `busy` — and they are joined with commas rather than one winning.
The template asked for all three; dropping two would be losing them silently.

## The hint

Android has a single hint slot and a reader speaks it on text fields, so it
always goes to `setHintText`. On something that is pressed, what a reader
actually reads out is the label of the click action, so the hint goes there too:

```java
info.setHintText(hint);
if (host.isClickable()) {
    info.addAction(new AccessibilityActionCompat(ACTION_CLICK, hint));
}
```

That is what turns "double tap to activate" into "double tap to save it without
leaving the screen".

## `accessible`, and the order that was a bug

`accessible: true` is `IMPORTANT_FOR_ACCESSIBILITY_YES` plus
`setScreenReaderFocusable(true)`: one stop instead of three for a row made of an
icon, a title and a subtitle. `accessible: false` is
`IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS` — plain `NO` would hide the
view and leave its children in the tree, hanging off the grandparent, and
decoration has to go whole.

What is not obvious is that `setContentDescription` is not only a setter: on a
view still on `AUTO` it promotes it to `YES`. Without that promotion, an
`an-view` with a name and nothing else — no touch, no role, no state — stays a
container the system finds uninteresting, and the name is read by nobody: the
node does not even appear in the tree a screen reader walks.

So the importance is written **first** and the name **after**. Written the other
way round the promotion is undone and the label goes mute. This was not caught
by reading the code; it was caught by the dump, and it is why the dump exists.

## What is not overwritten

An `an-button` is a `MaterialButton` and an `an-switch` is a `MaterialSwitch`.
Both already answer correctly: the button announces itself as a button, and the
switch says whether it is on. Writing our own role and state over them by
default would replace something that is right with something we guessed.

So every field of the node state is null until the template sets it, the
delegate calls the one underneath first, and only the fields that arrived get
written. A control with no accessibility prop reaches the accessibility tree
exactly as Material left it, and `check-a11y-device.sh` asserts that: it looks
in the dump for a `Switch` with an empty content description that reports itself
as on.

`testID` is the one collision. On iOS it is `accessibilityIdentifier`, a
separate field; on Android there is no second place — the `resource-id` that
would be its equivalent only takes integers from the app's `R`. So both end up
in `contentDescription`, and the rule is that the label wins: `testID` fills the
name only when there is no `accessibilityLabel`. Letting whichever arrived last
win would be an order the template does not control.

## Wear OS

Wear OS is Android, so all of it applies with nothing added: the same
`android.view.View`, the same `AccessibilityNodeInfo`, the same delegate. The
screen reader is TalkBack too. Nothing in this page is conditional on the shape
of the device.

That was checked and not assumed: the same APK built with `an wearos`, installed
on a Wear OS 5 emulator, dumps the same classes, the same content descriptions
and the same checkable/checked/selected/enabled for every row that fits on a
227-point round screen.

Three things are worth saying anyway. `check-a11y-device.sh` refuses to run on a
watch, and says why: this example lays out more rows than fit on a dial, what is
off screen is not in the accessibility tree either, and the check would fail on
the screen size while reading as if the labels had not arrived. The dumper on
the Wear image — Android 14 — does not write the `hint` attribute at all, where
Android 16's does; the check notices that and says the hint went unchecked
rather than failing on a tool that never reported it. And the primitives Wear OS
does not mount — `an-tab-bar`, `an-select` and the five others in
[docs/wearos.md](https://github.com/nesgarbo/angular-native/blob/main/docs/wearos.md) —
leave a visible marker in their place, and that marker keeps its own: it takes
no props of the primitive it replaces, accessibility included, precisely so it
cannot disguise itself as the control that is not there.

The crown is the one thing that could have needed something of its own and does
not: it is a rotary encoder, not a touch, so it never goes near the accessibility
actions.

## Seeing it, not claiming it

An accessibility prop that reaches nobody does not crash, does not log and
looks exactly like one that works. It can only be seen with a screen reader on,
or with a dump — and Android has a dump:

```bash
adb shell uiautomator dump --compressed /sdcard/tree.xml
adb shell cat /sdcard/tree.xml
```

That serialises the `AccessibilityNodeInfo` tree the platform built, which is
the same tree TalkBack walks. It is not our word for it.

The two flags matter and they are not the same tree:

- **plain** includes views the system does not consider important for
  accessibility, because UiAutomation asks for them so a UI test can reach
  anything;
- **`--compressed`** does not. That is the screen reader's tree, and it is where
  `accessible: false` has to have removed a row along with its contents.

```bash
./scripts/check-a11y.sh                       # text only, runs in check-all
./scripts/check-a11y-device.sh emulator-5554  # builds, installs, dumps, compares
```

`check-a11y-device.sh` builds `examples/a11y`, installs it, waits for the screen,
dumps both trees and compares them against what the template asked for. The
expectations are not written in the check: they are read out of the example's
template and out of the role table in the host's own Java, so the check cannot
say the template asked for something it did not.

It is out of `check-all.sh` because it needs a device and around two minutes.
The half that costs nothing — that the contract, the host, the strings and this
page all still say the same thing — is hooked up there.

## What the dump cannot show

`uiautomator dump` shows `content-desc`, `hint`, `class`, `checkable`,
`checked`, `selected`, `enabled` and `clickable`, and whether a node is in the
tree at all. That covers the label, the hint, the seven roles that map to a
class, `none`, the whole of `checked`/`selected`/`disabled`, and both halves of
`accessible`.

It does not show `roleDescription`, `heading`, `stateDescription` or the labels
of actions. So `link`, `search`, `summary`, `header`, `accessibilityValue`,
`busy`, the "partially checked" of `mixed` and the hint of a pressable node are
set and unit-checked in Java, but not proven by the dump. Proving them needs a
running screen reader reading out loud, which is a different kind of check and
is not written.
