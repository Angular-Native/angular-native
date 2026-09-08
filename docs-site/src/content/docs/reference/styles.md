---
title: Styles and layout
description: The flexbox subset the Rust core resolves, the values it accepts, and what a style name it does not know does.
sidebar:
  order: 2
---

Layout is not the platform's. Flexbox runs once, in the core, over
[taffy](https://github.com/DioxusLabs/taffy), and every host is handed absolute
frames in logical points. So the set of styles below is the same on a phone, a
TV and a watch — a template that lays out one way lays out that way everywhere.

There is **no cascade, no inheritance, no selectors and no stylesheets**. A
style is written on the node it applies to:

```html
<an-view [style.flexDirection]="'row'" [style.gap]="'12'" [style.padding]="'16'">
  <an-view [style.flex]="'1'"><an-text>left</an-text></an-view>
  <an-view [style.width]="'80'"><an-text>right</an-text></an-view>
</an-view>
```

Values are strings because that is what Angular's `[style.x]` binding produces.
The core parses them once, on assignment.

## What the core accepts

51 names, and nothing else. They are declared in
`crates/an-layout/src/style.rs` and mirrored on the TypeScript side in
`packages/platform-native/src/style-names.ts`; `scripts/check-styles.sh` fails
if the two lists drift apart.

### Box and flow

| Style | Values | Default |
|---|---|---|
| `display` | `flex`, `none` | `flex` |
| `position` | `relative`, `absolute` (`static` reads as relative, `fixed` as absolute) | `relative` |
| `overflow` | `visible`, `hidden`, `scroll` | `visible` |
| `boxSizing` | `border-box`, `content-box` | `border-box` |

`overflow` sets both axes at once — there is no `overflowX`.

### What clipping actually does

`hidden` and `scroll` both **clip**: the node's box is the whole of what it may
paint, and a child that sticks out is cut off at the edge rather than drawn over
whatever is next to it. `visible`, the default, lets it through.

Two nodes clip whatever the template says:

- **The root.** It is the app's body. Nothing an app contains may be painted
  outside the window — a body that does not clip is a canvas, and content pushed
  past its edge goes on existing off-screen.
- **Anything scrollable.** Otherwise its content is drawn over the things around
  it as soon as there is more of it than fits.

:::caution[Two hosts only ever tighten]
On Android and on watchOS, containers have always clipped unconditionally, and
`overflow: visible` has never worked there. What arrives from layout can
therefore only turn clipping **on**, never off: honouring `visible` on those two
now would not be obeying a style, it would be a new behaviour arriving as the
side effect of a bug fix, and every layout built against the old one would start
leaking children. On iOS, tvOS, visionOS and macOS the resolved value is applied
as it stands.
:::

### One axis, and only one

A scrollable node's content is **clamped to its own frame across the axis it is
not scrolling on** — in the core, and again in the host where the scrolling
happens. Which axis that is comes from `[horizontal]` on `<an-scroll-view>`;
with nothing said it is downwards.

That clamp is not tidiness. A `UIScrollView` scrolls on whichever axis its
content is bigger, so content a few points too wide — one image reporting its
intrinsic size into a row, say — turns the whole page into something that can be
dragged sideways until there is nothing on screen. Along the axis that was asked
for, overflow is the point; across it it is always a mistake somewhere else, and
it should show up as a clipped edge rather than as a screen that can be swiped
away.

`scripts/check-clip.sh` asserts that the root and every scrollable node clip and
that an ordinary container does not; `scripts/check-scroll.sh` asserts that no
scroll view is bigger inside than out across the axis it scrolls on.

### Flex

| Style | Values | Default |
|---|---|---|
| `flexDirection` | `row`, `column`, `row-reverse`, `column-reverse` | **`column`** |
| `flexWrap` | `nowrap`, `wrap`, `wrap-reverse` | `nowrap` |
| `justifyContent` | `flex-start`, `flex-end`, `center`, `space-between`, `space-around`, `space-evenly`, `stretch` | unset |
| `alignItems` | `flex-start`, `flex-end`, `center`, `stretch`, `baseline` | unset |
| `alignSelf` | same as `alignItems` | unset |
| `alignContent` | same as `justifyContent` | unset |
| `flex` | a number | — |
| `flexGrow` | a number | `0` |
| `flexShrink` | a number | **`0`** |
| `flexBasis` | points, percentage or `auto` | `auto` |

Two defaults are React Native's rather than CSS's, and both matter: children
stack **downwards** unless told otherwise, and nothing shrinks below its size
unless asked to.

`flex: N` is the shorthand, and it means what it means in CSS: grow `N`, shrink
`1`, basis `0`. It is what almost everybody writes instead of the three
separately.

`justifyContent` and `alignContent` accept the same keyword set; `alignItems`
and `alignSelf` accept `baseline` but not the `space-*` family.

### Size

| Style | Values |
|---|---|
| `width`, `height` | points, percentage, `auto` |
| `minWidth`, `minHeight`, `maxWidth`, `maxHeight` | points, percentage, `auto` |
| `aspectRatio` | a number — width divided by height |

An unset maximum is `auto`, not zero. A `maxWidth` of zero would leave the view
with no size at all, which is the opposite of "there is no maximum".

### Spacing

| Style | Values |
|---|---|
| `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft` | points, percentage, `auto` |
| `marginHorizontal`, `marginVertical` | the same, two edges at a time |
| `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft` | points, percentage |
| `paddingHorizontal`, `paddingVertical` | the same, two edges at a time |
| `gap`, `rowGap`, `columnGap` | points, percentage |

`marginStart` and `marginEnd` are accepted as spellings of `marginLeft` and
`marginRight`, and the same for padding. They are **not** direction-aware: this
is not a right-to-left layout engine, and the names are taken only so that a
template written with them does not silently do nothing.

Margins take `auto`; padding and gap do not.

### Border width and offsets

| Style | Values |
|---|---|
| `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth` | points, percentage |
| `top`, `right`, `bottom`, `left` | points, percentage, `auto` |

Border **width** is layout, so it lives here. Border colour and radius are not
— they are props on the primitive, along with `backgroundColor`. See
[Components](/reference/components/).

#### There is one border, and it is a prop

Two things are spelled almost alike and do opposite jobs, so it is worth being
blunt about which is which:

- `[borderWidth]`, the **prop**, is the line. One number for all four sides,
  drawn by every host inside the frame, over the content. It moves nothing.
- `borderWidth` and the four per-side widths, the **styles** in the table above,
  are layout. They inset the children and they stop in the core: no host is ever
  told about them, so no line comes of them.

The uniform style has a use, and it is the one CSS gives it: written next to the
prop, `[style.borderWidth]="'2'"` reserves the two points the prop's line is
painted over, so the line does not sit on top of a child.

The four per-side ones have no such use, because there is no per-side line to
reserve room for. `[style.borderTopWidth]="'2'"` pushes the children down by two
points and draws nothing, on every platform. That used to happen in silence; the
core now says it once per name:

```text
angular-native: [style.borderTopWidth] insets this node's children and draws
nothing. The border that is drawn is the [borderWidth] prop — one number, all
four sides, on every platform — and there is no per-side one to reserve room
for. If the inset is what was wanted, it is padding.
```

They are not drawn and they are not going to be. `CALayer` on UIKit and AppKit
has one border, `GradientDrawable` on Android has one stroke, and SwiftUI on the
watch has no view at all — four hosts would each have to invent the same drawing
from scratch, corner radii and all, and a per-side border that got the corners
wrong would be worse than none. The names stay recognised so that a template
written with them keeps its layout and gets told what it did not get, rather
than being told the style does not exist.

## Values

| Written | Read as |
|---|---|
| `'16'`, `'16px'` | 16 logical points |
| `'50%'` | half of the parent's corresponding dimension |
| `'auto'` | the prop's automatic behaviour |
| `'row'`, `'center'`, … | a keyword, where the prop takes one |
| `''` | unset — back to the default |

Units are **logical points**, not physical pixels. The device's scale factor
never enters a template; it is the platform's business when it draws.

Both spellings of every hyphenated keyword work: `space-between` and
`spaceBetween`, `flex-start` and `flexStart` (and `start`), `column-reverse` and
`columnReverse`. Angular hyphenates style *names* before handing them over, so
`[style.flexDirection]` and `[style.flex-direction]` are the same thing to the
core.

A value the core cannot parse becomes `Unset` — the prop falls back to its
default rather than to whatever it held before.

## Font styles are not styles

Nine names look like CSS but are **props**, not layout:

`fontSize` · `fontWeight` · `fontStyle` · `fontFamily` · `lineHeight` ·
`letterSpacing` · `color` · `textAlign` · `numberOfLines`

The core needs them to *measure* text and the host needs them to *draw* it, so
they travel as props on the node. Writing them as `[style.fontSize]` works —
the renderer recognises the nine and reroutes them — but the typed input
`[fontSize]` is what the template compiler can check, and it is what to write.

Before that reroute existed, `[style.fontSize]` did nothing at all: the text was
measured in the default font and drawn in UIKit's, so it spilled out of a box
that had been sized for something smaller and the parent clipped it. It looked
like text disappearing.

## An unknown style says so

Angular accepts `[style.whatever]` without a murmur. So the renderer checks the
name against the list above and warns, **once per name**, when nobody is going
to look at it — naming the style, saying it will do nothing, and pointing at
where it probably belonged: a property of the control, such as a colour, a
title or a value, is a typed input and not a style.

Once per name and not once per write: the same style is set again on every
change detection pass, and warning every time would fill the log without saying
anything new.

This is the shape the whole framework takes on a gap. A style that travels, that
no host reads and that raises nothing is the most expensive kind of bug there
is: it looks like the feature simply does not work, and there is nothing to
grep for.

## Classes and `!important`

`addClass` accumulates names and sends them as a `className` prop. Nothing in
the core or the hosts resolves them — there are no stylesheets — so a class is
inert unless a styling layer above does something with it.

`RendererStyleFlags2.Important` is ignored and the value applied. Without a
cascade there is nothing for `!important` to win against.

## Measuring: where a size comes from when you do not give one

A node with no explicit size is measured by asking the platform, through the
`TextMeasurer` trait. There are three kinds of leaf:

- **Text** — measured in the system's real typeface, with the font props above,
  against the available width. The intrinsic minimum gets a separate question:
  asking for it as "available width zero" looks equivalent and is not — both
  platforms answer zero, and then the text shrinks to nothing as soon as nobody
  imposes a width.
- **Controls** — a `UISwitch`, a `MaterialButton`. Each is measured once at
  startup from a sample control, because creating one needs the main thread and
  the measurer lives on the engine's.
- **Images** — from the intrinsic size, keeping the aspect ratio when only one
  dimension is fixed.

A node with a measure function is a **leaf**: layout does not descend below it,
children or no children.

That startup measurement is worth knowing about when a control is asked to hold
more than the sample did. A button with an `[ios].subtitle` is two lines tall
and the sample was one, so it needs an explicit height in the template or the
title gets clipped.

## `contentSize`, and the axis it travels on

For a scrollable node the core also reports how much room the children take,
which is what a `UIScrollView` wants as its `contentSize`.

The op carries two numbers and no axis, so the host is told which one it is by
the `horizontal` prop and clamps the other to the view's own frame. A scroll
view that is bigger inside than out on both axes is not a feature this engine
offers: it is what a stray intrinsic size looks like.

### A size on a scroll view is a size

A scroll view must not be sized by its content — a list of five thousand rows
would make it five thousand rows tall — and in flexbox that is said with
`flex-basis: 0`. A non-`auto` `flex-basis` also beats `width` and `height` on
the main axis, which is how a `[style.height]` on an `<an-scroll-view>` used to
resolve to nothing at all.

The basis is therefore resolved per layout, against the direction the *parent*
lays its children out in: it is the size the template asked for on that axis
whenever it asked for one, and zero only when it did not.
