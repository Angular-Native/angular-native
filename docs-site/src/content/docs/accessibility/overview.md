---
title: The contract
description: Six props, one vocabulary, and three very different platform APIs underneath — what each one is for, what you get for free, and why the checks read the tree back instead of asserting the props were set.
sidebar:
  order: 1
---

Most of accessibility here is not yours to do. `an-switch` **is** a `UISwitch`
and a `MaterialSwitch`, so VoiceOver already calls it a switch, already says
whether it is on, already responds to the double-tap, and already grows with the
user's text size. That is the single largest argument for using the platform's
controls rather than drawing lookalikes: a drawn switch answers the
accessibility API with silence.

What is left is the part no system can infer — what an `an-view` acting as a
button is *called*, and what state it is in. Six props cover it, they are the
same six on all eight platforms, and they live on the base class so every
primitive has them.

| Prop | Type | What it answers |
|---|---|---|
| `accessibilityLabel` | `string` | What is it called? |
| `accessibilityRole` | `NativeRole` | What is it? |
| `accessibilityValue` | `string` | What is it worth right now? |
| `accessibilityState` | `NativeAccessibilityState` | What state is it in? |
| `accessibilityHint` | `string` | What happens if I activate it? |
| `accessible` | `boolean` | Is this one element, or a container to walk into? |

```html
<an-view
  [accessibilityRole]="'button'"
  [accessibilityLabel]="'Delete this note'"
  [accessibilityHint]="'It cannot be undone'"
  [accessibilityState]="{ disabled: saving() }"
  (press)="remove()">
  <an-icon [name]="'delete'" />
</an-view>
```

Without the label that view is an element with no name: you can focus it, and
you cannot tell what it does. The icon inside does not help — an icon has no
text.

## The vocabulary

**Roles** — twelve, and no more. A name outside the list is ignored rather than
being turned into a role that gets applied:

`button` · `link` · `header` · `image` · `text` · `checkbox` · `radio` ·
`switch` · `slider` · `search` · `summary` · `none`

**State** — five optional fields, each of which may simply be absent:

```ts
{ disabled?: boolean
  selected?: boolean
  checked?: boolean | 'mixed'
  expanded?: boolean
  busy?: boolean }
```

State is kept apart from the role because it changes over time and the role does
not. A screen reader announces "selected" again when this changes, with no view
being rebuilt.

`checked` takes `'mixed'` as well as a boolean, because a three-state checkbox is
a real control on both platforms and collapsing it to `false` would say the wrong
thing.

## `accessible` is the one people miss

It decides whether a subtree is **one stop** for a screen reader or several.

```html
<an-view [accessible]="true" [accessibilityLabel]="'Ada Lovelace, 3 unread'">
  <an-icon [name]="'account'" />
  <an-text>Ada Lovelace</an-text>
  <an-text>3 unread</an-text>
</an-view>
```

Without it that row is three separate stops, and getting through a list of forty
of them means a hundred and twenty swipes. With it, one stop, read out in one
go.

`false` does the opposite: it hides the view **and everything inside it**, which
is what purely decorative things need — a divider, a background image, a
gradient.

## Three APIs, one contract

The prop names are the contract; what they become is each platform's business.

| | The API underneath |
|---|---|
| **iOS · iPadOS · tvOS · visionOS** | `UIAccessibility` — traits, labels, values on `UIView` |
| **macOS** | `NSAccessibility` — a protocol on `NSView`, with a different role vocabulary |
| **watchOS** | SwiftUI's `.accessibilityLabel`, `.accessibilityValue`, `.accessibilityAddTraits` |
| **Android · Wear OS** | `AccessibilityNodeInfo`, populated in two halves |

They do not line up neatly, and where they do not, each platform's page says so
rather than pretending:

- [Accessibility on Apple](/accessibility/apple/) — the three Apple shapes, role
  by role and state by state, and what has no equivalent on each.
- [Accessibility on Android](/accessibility/android/) — the two halves of
  `AccessibilityNodeInfo`, the role mapping, and the ordering bug that `accessible`
  turned out to have.

## `testID` is not one of the six

```html
<an-view [testID]="'save-button'" />
```

It ends up in `accessibilityIdentifier`, which is what UI-test frameworks look
for. It is not read out by anything and it is not a label — a view with a
`testID` and no `accessibilityLabel` is still nameless to a screen reader.

## The checks read the tree back

`scripts/check-accessibility.sh` and `scripts/check-a11y.sh` do **not** assert
that the props were set. That would only prove the template says what the
template says.

They install the app and ask the system for its accessibility tree — through the
same API VoiceOver and TalkBack use — and compare what a screen reader would
actually be told. Then they compare it against what the template asked for.

That is a different claim and the only one worth making. A prop that travels to
a host which does not read it looks identical to one that works, right up until
somebody with a screen reader tries the app.

The half that costs nothing runs in `check-all.sh`; `check-a11y-device.sh` is the
half that installs an APK on a real phone. See
[checking it without a device](/guide/testing/).

## What the dump cannot show

Reading the tree proves the label is there and says the right words. It does not
prove the order is sensible, that the focus goes somewhere useful after a modal
closes, or that the hint is worth reading. Those need a person and a screen
reader, and both platform pages say which of their claims are of that kind.
