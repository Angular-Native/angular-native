---
title: Icons
description: SF Symbols on Apple platforms and Material Symbols on Android, asked for by name — the shared vocabulary, when to write the native name instead, and why one of the two is bundled and the other is not.
sidebar:
  order: 6
---

```html
<an-icon [name]="'settings'" [size]="28" [color]="'#f4f7ff'" />
```

That is an SF Symbol on iOS, on the TV, in the headset and on the watch, and a
Material Symbol on Android and on Wear OS. Nothing is drawn here and no icon is
traced by hand: the name goes to the system, and the system supplies the glyph
with whatever weight and stroke that version of the OS gives it — which means
the icon ages with the platform instead of staying pinned to the day it went
into the project.

## The inputs

| Input | Type | Default | |
|---|---|---|---|
| `name` | `string` | — | A shared name, or the platform's own. |
| `size` | `number` | `24` | Points. It also **sets the view's width and height**. |
| `weight` | `number` | unset | The stroke, on the typographic scale: `100`–`900`. |
| `color` | `string` | unset | |

`size` is not only a size. On Apple platforms a large symbol is not the small
one scaled up, it is a different drawing, so the number is handed to
`UIImageSymbolConfiguration` and the system picks the stroke that belongs with
it. It is also written out as `width` and `height` styles, because layout has to
know how big the box is — without that an `<an-icon>` with nothing else on it
would be measured at zero and never appear.

`weight` maps onto the platform's own scale:

| `weight` | SF Symbol weight |
|---|---|
| `100`–`299` | Light |
| `300`–`499` | Regular |
| `500`–`599` | Medium |
| `600`–`699` | Semibold |
| `700` and up | Bold and heavier |

## One name, two icon sets

Thirty-two common names — the ones almost every app carries in a tab bar or a
header — are translated into each platform's own, so the same template works on
both:

| Name | SF Symbol | Material Symbol |
|---|---|---|
| `home` | `house.fill` | `home` |
| `search` | `magnifyingglass` | `search` |
| `settings` | `gearshape.fill` | `settings` |
| `profile` · `account` | `person.crop.circle.fill` | `account_circle` |
| `back` | `chevron.left` | `arrow_back` |
| `forward` | `chevron.right` | `arrow_forward` |
| `close` | `xmark` | `close` |
| `add` | `plus` | `add` |
| `remove` | `minus` | `remove` |
| `delete` | `trash` | `delete` |
| `edit` | `pencil` | `edit` |
| `share` | `square.and.arrow.up` | `share` |
| `favorite` | `heart.fill` | `favorite` |
| `star` | `star.fill` | `star` |
| `menu` | `line.3.horizontal` | `menu` |
| `more` | `ellipsis` | `more_horiz` |
| `check` | `checkmark` | `check` |
| `info` | `info.circle` | `info` |
| `warning` | `exclamationmark.triangle.fill` | `warning` |
| `refresh` | `arrow.clockwise` | `refresh` |
| `calendar` | `calendar` | `calendar_month` |
| `camera` | `camera.fill` | `photo_camera` |
| `bell` | `bell.fill` | `notifications` |
| `chat` | `bubble.left.fill` | `chat_bubble` |
| `mail` | `envelope.fill` | `mail` |
| `list` | `list.bullet` | `list` |
| `play` | `play.fill` | `play_arrow` |
| `pause` | `pause.fill` | `pause` |
| `download` | `arrow.down.circle` | `download` |
| `upload` | `arrow.up.circle` | `upload` |
| `location` | `location.fill` | `location_on` |
| `lock` | `lock.fill` | `lock` |

The vocabulary is Material's naming, which is why the Android side only has to
translate the ten that genuinely differ and lets the rest through untouched.

## The table is a shortcut, not an allowlist

**Anything not in it passes through unchanged.** That is the important half:

```html
<an-icon [name]="'figure.run'" />        <!-- an SF Symbol, written directly -->
<an-icon [name]="'directions_run'" />    <!-- a Material Symbol, written directly -->
```

There are more than five thousand SF Symbols and about as many Material Symbols.
Duplicating either list here would make no sense, and it would turn a shortcut
into a gate. The thirty-two are the ones worth not writing twice; everything else
is one line in each of two templates, or a small map of your own.

A name the platform's set does not have renders **nothing**, silently: iOS gets
no `UIImage` back and Android finds no codepoint, and neither of them logs it.
The box is still there — `size` gave it width and height — it is just empty. If
an icon is missing on one platform and not the other, a misspelling or a name
that only exists in one of the two sets is the first thing to check.

## Nothing is bundled on Apple. Something is on Android.

**On Apple platforms** the symbol is `UIImage.systemImageNamed`. The set is part
of the OS, it grows with each release, and it already respects the user's
accessibility text size. The app carries no icon assets at all.

**On Android** the Material Symbols font *is* bundled —
`material-symbols.ttf` and its codepoint table, in the shell's assets — and the
icon is drawn as a glyph through a `Typeface`.

That asymmetry is not a preference. What Android ships in the platform has been
frozen since 2011: `android.R.drawable` is the 2.x icon set, and it is not
Material 3's. An app that used it would look like an app from another decade next
to the switch and the button beside it, both of which *are* Material 3. So the
current set travels with the app.

It is the one place in this project where something is carried rather than asked
for, and it is carried for the same reason everything else is asked for: so the
result matches what the platform draws today.

:::note[Where the icon is a `Drawable`, not a view]
`an-button`'s `[icon]` goes through the same table, but Android's
`MaterialButton` wants a `Drawable` rather than a glyph, so the font is rendered
into one. The name you write is the same either way.
:::

## The table lives in three places

`crates/an-core/src/icons.rs` is the shared copy, `crates/an-ios/src/icons.rs`
has one for the UIKit hosts, and `AnHost.java` has the ten Android divergences.
Unlike the style names and the primitive names, **no check script compares
them** — see [checking it without a device](/guide/testing/#the-lists-that-must-not-drift)
for what that guard looks like where it does exist.

If you add a common name, add it in all three.
