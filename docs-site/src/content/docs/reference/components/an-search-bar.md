---
title: "an-search-bar"
description: "The platform's search field, with its magnifier, its clear button and its cancel affordance."
sidebar:
  order: 14
---

`UISearchBar`, Android's `SearchView`, `NSSearchField`. It is not an
`an-text-input` with an icon: a search field has its own accessibility role, its
own clear behaviour and, on iOS, its own relationship with the navigation bar.

It reports `(input)` as you type and `(submit)` on the return key, which is the
distinction that matters when the search costs a network round trip.

## Availability

| iOS · iPadOS | Android | macOS | tvOS | visionOS | watchOS | Wear OS |
|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ | ✓ | — | — |

## What it becomes

| iOS · iPadOS | Android | macOS | watchOS |
|---|---|---|---|
| `UISearchBar` | `SearchView` | `NSSearchField` | — |

## Inputs

| Prop | Type | Default | |
|---|---|---|---|
| `value` | `string \| null` | `''` |  |
| `placeholder` | `string \| null` | `null` |  |

## Outputs

| Output | Payload | |
|---|---|---|
| `(input)` | `{ value: string }` |  |
| `(submit)` | `{ value: string }` |  |

## Example

```html
<an-search-bar
  [value]="query()"
  [placeholder]="'Search'"
  (input)="query.set($event.value)"
  (submit)="run($event.value)" />
```

Every primitive also carries the base props — background, border, opacity, the transforms, `animate`, the accessibility contract — and every gesture. See [Components](/reference/components/) for the full base, [Events](/reference/events/) for the payloads.
