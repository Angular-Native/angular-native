---
title: "an-tab-bar"
description: "The bottom navigation bar — a real UITabBarController, a BottomNavigationView, and on the Mac the control Mac apps actually use."
sidebar:
  order: 15
---

`UITabBarController` on iOS, `BottomNavigationView` on Android. On the Mac it is
an `NSSegmentedControl`, because a Mac app does not have a bottom tab bar and
putting one there would be a phone app in a window.

It is a control, not a router outlet. It reports `(select)` with an index and you
decide what that means — swapping a signal, or navigating. It does not own a
route stack per tab the way `UITabBarController` does natively.

`[icons]` takes the same names as `an-icon`.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | — | — |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UITabBarController` | `BottomNavigationView` | `NSSegmentedControl` | — |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `items` | `readonly string[] \| null` | `null` |  |
| `icons` | `readonly string[] \| null` | `null` | Icons, in the same order as the titles. |
| `selectedIndex` | `number \| null` | `0` |  |
| `color` | `string \| null` | `null` | The active tab's colour. |
| `unselectedColor` | `string \| null` | `null` | The colour of the rest. |

## Outputs

| Output | Payload | |
|---|---|---|
| `(select)` | `number` |  |

### `[ios]` — what UIKit has and the others do not

| Key | Type | |
|---|---|---|
| `translucent` | `boolean` | Whether what goes past behind it shows through. `UITabBar.isTranslucent`. Material's bar is opaque by design and has no switch for this. |

## Example

```html
<an-tab-bar
  [items]="['Home', 'Search', 'Profile']"
  [icons]="['home', 'search', 'profile']"
  [selectedIndex]="tab()"
  (select)="tab.set($event)" />
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
