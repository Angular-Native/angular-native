---
title: The binary protocol
description: Twelve opcodes, little-endian, one buffer per tick — the wire format between JavaScript and the Rust core, the node codes, and what happens when a command is turned down.
sidebar:
  order: 8
---

JavaScript does not call into Rust once per mutation. It writes commands into a
buffer and hands the whole thing over at the end of the tick.

The reason is arithmetic. A `@for` over 200 rows is around 1,200 mutations.
One call each is 1,200 border crossings; a buffer is one.

Most people never see this page — `Renderer2` writes the buffer and the core
reads it. It is here for anyone debugging a mutation that did not land, writing
a host, or wondering what "the protocol" means when a page says a name is frozen
by it.

## The shape

All little-endian. Every command is an opcode byte followed by its operands.
Strings carry a `u32` byte length and then UTF-8, with **no alignment**: the
decoder reads byte by byte and does not need any.

| Type | Bytes |
|---|---|
| opcode | `u8` |
| node id | `u32` |
| index | `u32` |
| number | `f64` |
| boolean | `u8`, zero or not |
| string | `u32` length, then that many UTF-8 bytes |

## The twelve opcodes

| Code | Command | Operands |
|:--:|---|---|
| `0x01` | `CREATE_NODE` | id `u32`, kind `u8` |
| `0x02` | `DESTROY_NODE` | id `u32` |
| `0x03` | `INSERT_CHILD` | parent `u32`, child `u32`, index `u32` |
| `0x04` | `REMOVE_CHILD` | parent `u32`, child `u32` |
| `0x05` | `SET_STYLE` | id `u32`, name `str`, value `str` |
| `0x06` | `SET_PROP_STR` | id `u32`, key `str`, value `str` |
| `0x07` | `SET_PROP_NUM` | id `u32`, key `str`, value `f64` |
| `0x08` | `SET_PROP_BOOL` | id `u32`, key `str`, value `u8` |
| `0x09` | `SET_PROP_NULL` | id `u32`, key `str` |
| `0x0A` | `SET_TEXT` | id `u32`, text `str` |
| `0x0B` | `SET_LISTENER` | id `u32`, event `str`, enabled `u8` |
| `0x0C` | `SET_ROOT` | id `u32` |

Styles travel as strings and are parsed by the core on assignment, because that
is what Angular's `[style.x]` binding produces. Props are typed on the wire —
four opcodes rather than one — so that a number does not have to be reparsed on
the other side, and so that `null` means *unset* rather than the string
`"null"`.

`SET_LISTENER` is what makes a gesture cost nothing until it is asked for: it
arrives when Angular subscribes to an output, and the host attaches the
recogniser then.

## Node ids are assigned by JavaScript

Creating a node needs no round trip. JS picks the id, writes `CREATE_NODE`, and
carries on referring to it — the same as Fabric's tags. There is no allocation
call to wait for and no id table to reconcile.

## The node kinds

| Code | Kind | Code | Kind |
|:--:|---|:--:|---|
| `0` | `View` | `13` | `Modal` |
| `1` | `Text` | `14` | `Alert` |
| `2` | `RawText` | `15` | `Icon` |
| `3` | `Image` | `16` | `SegmentedControl` |
| `4` | `ScrollView` | `17` | `Stepper` |
| `5` | `TextInput` | `18` | `SearchBar` |
| `6` | `StackView` | `19` | **`Picker`** |
| `7` | `TabBar` | `20` | `DatePicker` |
| `8` | `Switch` | `21` | `NavigationBar` |
| `9` | `Slider` | `22` | **`TextEditor`** |
| `10` | `ActivityIndicator` | `23` | `WebView` |
| `11` | `ProgressBar` | `24` | `MapView` |
| `12` | `Button` | `25` | `VideoView` |

`RawText` is written in no template: it has neither a tag nor a directive. It is
the text node Angular creates for the characters inside an `<an-text>`.

The two in bold are the names the core cannot change. `an-select` travels as
`Picker` and `an-textarea` as `TextEditor` — chosen back when the tag could not
be called `Select` or `TextArea`, because Angular will not self-close anything
named like an HTML element. The prefix later gave the tags their names back, but
renaming the enum would mean changing the protocol, so the wire keeps the old
ones.

Everywhere else the name is derived by rule rather than by table: strip `an-`
and join in PascalCase, so `an-text-input` is `TextInput`. That rule spans four
files in three languages, and `scripts/check-kinds.sh` is what stops it drifting.

## When a command is turned down

`apply()` returns how many commands it applied, or one of five errors:

| Error | Meaning |
|---|---|
| `Truncated { offset }` | The buffer ran out halfway through a command. |
| `UnknownOpcode { opcode, offset }` | |
| `UnknownKind { kind, offset }` | A `CREATE_NODE` with a code past 25. |
| `InvalidUtf8 { offset }` | |
| `Tree { opcode, offset, error }` | The core refused the mutation: no such node, duplicate id, and so on. |

Every one of them carries the **offset**, and `Tree` carries the **opcode** as
well. That is the difference between "the buffer was bad" and "the
`INSERT_CHILD` at byte 412 named a parent that does not exist", and it is the
whole reason the errors are shaped this way.

:::caution[There is no rollback]
An error leaves the tree with everything applied up to that point. That is
deliberate: a malformed buffer is a bug on the JS side, not an expected
condition, and leaving it half-applied puts the failure on the screen where
somebody will see it rather than hiding it behind a clean revert.
:::

## The encoder in Rust

`an-bridge` also contains an encoder for the same format. The runtime does not
use it — the prelude writes the real buffers in JavaScript.

It exists so the decoder can be tested without starting a JS engine, and so that
any change to the format breaks the tests at **both** ends at once rather than
letting the two sides drift until something renders wrong on a device.

## Where each half lives

| | |
|---|---|
| The writer | `packages/runtime/runtime.js` — the prelude's command buffer |
| What calls it | `packages/platform-native/src/renderer.ts` and `native-node.ts` |
| The reader | `crates/an-bridge/src/protocol.rs` |
| What it applies to | `crates/an-core/src/tree.rs` — the shadow tree |

One tick of Angular fills the buffer; `commit()` decodes it, resolves the layout
over taffy, diffs against the previous frame and produces the `MountOp` list the
host applies. That whole path is described in
[how it works](/guide/how-it-works/).
