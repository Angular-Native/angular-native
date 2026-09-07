---
title: Long lists
description: "`an-virtual-list` mounts a fixed number of views and never creates another one while you scroll — how the recycling works, why the row height has to be declared, and when a plain `@for` is the better answer."
sidebar:
  order: 7
---

A `@for` over ten thousand rows creates ten thousand native views. On a phone
that is not slow, it is fatal: the memory is real `UIView`s and real
`ViewGroup`s, and the commit that mounts them is one frame long enough to be a
freeze.

`an-virtual-list` mounts as many rows as fit on screen plus a margin, and
scrolling creates and destroys **none** of them. It changes what each one shows
and where it sits. Ten thousand rows cost the same twenty native views as twenty
rows do.

```html
<an-virtual-list [items]="rows()" [itemHeight]="64" [style.flexGrow]="'1'">
  <ng-template let-row let-i="index">
    <an-view [style.padding]="'16'" [style.gap]="'4'">
      <an-text [fontSize]="16">{{ row.name }}</an-text>
      <an-text [fontSize]="13" [color]="'#8a93a6'">row {{ i }}</an-text>
    </an-view>
  </ng-template>
</an-virtual-list>
```

```ts
import { VirtualList } from '@angular-native/primitives'
```

## How the recycling works

The list keeps a fixed carousel of slots. Each slot has a `key` that is **its
position in the carousel**, not the item it happens to be showing, and the
template is rendered with `track slot.key`.

That one detail is the whole mechanism. Because the key does not change when you
scroll, Angular does not destroy and recreate the embedded view — it updates its
bindings. `NgTemplateOutlet` behaves the same way as long as the context object's
keys stay the same, which they do here: every context is `{ $implicit, index }`.

So a scroll of two hundred rows produces two hundred *prop updates* rather than
two hundred create-and-destroy pairs. On the wire that is the difference between
a handful of `SET_PROP` opcodes and a few thousand `CREATE_NODE`/`DESTROY_NODE`
ones.

Inside, the rows are absolutely positioned inside one tall spacer view, so the
scroll view's content size is the full height of the list from the first frame —
the scrollbar is honest and the scroll does not grow under your finger.

## The row height has to be declared

There is no way to know what sits at offset 12,000 without having measured
everything above it. So the height is an input, not a measurement:

```html
<an-virtual-list [items]="rows()" [itemHeight]="64">
```

With one number for every row nothing needs storing: row `i` starts at
`i * height`, and the row at a given offset comes out of a division.

For rows that differ, `itemHeight` takes a function instead:

```ts
protected readonly rowHeight = (row: Row, index: number) => (row.expanded ? 128 : 64)
```

```html
<an-virtual-list [items]="rows()" [itemHeight]="rowHeight">
```

The function is called **once per row every time the list changes**, not on every
scroll. The offsets are accumulated into a prefix-sum array and then searched by
bisection, which over five thousand rows is thirteen comparisons.

:::caution[A height that lies is a gap]
Nothing measures the row afterwards to check. If the template renders taller
than the number says, rows overlap; shorter, and there is a stripe of background
between them. The height is a contract.
:::

## The inputs

| Input | Type | Default | What it does |
|---|---|---|---|
| `items` | `readonly T[]` | required | |
| `itemHeight` | `number` \| `(item, index) => number` | required | |
| `overscan` | `number` | `4` | Spare slots at each end, so a fast flick leaves no gap. |
| `refreshing` | `boolean` | `false` | Whether the pull-to-refresh spinner is open. |

| Output | Payload | |
|---|---|---|
| `refresh` | — | Pull to refresh. With nobody listening, the gesture does not exist. |

Pull-to-refresh is one-way in and one-way out: the gesture opens the spinner and
emits `refresh`, and setting `[refreshing]` back to `false` is what closes it.
The list does not decide when your data has arrived.

```html
<an-virtual-list
  [items]="rows()"
  [itemHeight]="64"
  [refreshing]="loading()"
  (refresh)="reload()"
  [style.flexGrow]="'1'">
```

## Give it room

Both the list and the scroll view inside it have to be told they may be smaller
than their content, or they push the layout open and there is nothing left to
scroll. The component sets `minHeight: 0`, `flexBasis: 0`, `flexShrink: 1` and
`overflow: hidden` on its own host, but the **parent** still has to give it a
share of the space:

```html
<an-view [style.flexGrow]="'1'">
  <an-virtual-list [items]="rows()" [itemHeight]="64" [style.flexGrow]="'1'" />
</an-view>
```

A list with no `flexGrow` in a column that has other children will be measured
at its content height, which is the full ten thousand rows, and then there is no
viewport to recycle against.

## When not to use it

A plain `@for` inside an `an-scroll-view` is the right answer more often than
people expect. It costs one native view per row, which is fine up to a few
hundred, and in exchange:

- rows can be any height, measured rather than declared,
- there is no carousel to reason about,
- a row keeps its own state across scrolls, because it is never recycled.

The rule of thumb: if the number of rows is bounded by something a person typed
or chose, use `@for`. If it is bounded by what a server has, use the virtual
list.

## Watch what the template captures

A recycled row's template is re-evaluated with a new context, so anything derived
from the row has to be derived **in the template or from the context**, not
stored in a field of a child component. A child component inside the row keeps
its instance across the recycle, since that is the point — so a component that
loads something in its constructor based on the row it saw first will keep
showing that first row's data.

Give such a row a real input and derive from it, and the recycle is invisible.

## The example

```bash
cargo an dev examples/kitchen
```

`examples/kitchen` carries a five-thousand-row list with two row heights mixed
together. `scripts/check-list.sh` drives it through `headless` and asserts the
two numbers that matter:

```text
ok   scrolling recycles: not one new view
ok   69 native views for 5000 rows
```

A regression that reintroduces per-row creation fails there, in a second, rather
than on a device with a long list.
