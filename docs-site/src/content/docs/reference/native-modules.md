---
title: Native modules
description: How JavaScript calls native code, what crosses the boundary, what ships built in — and which platforms register nothing at all.
sidebar:
  order: 4
---

A primitive is a view. A **native module** is a method: JS asks for something,
native code answers, and the answer comes back as a promise. It is the same
mechanism a [plugin](/extending/plugins/) uses — a plugin is a native module
registered from outside this repository.

## Calling one

```ts
import { inject } from '@angular/core'
import { NativeModules } from '@angular-native/platform'

export class Battery {
  private readonly modules = inject(NativeModules)

  level(): Promise<number> {
    return this.modules.call<number>('battery', 'level')
  }
}
```

`NativeModules` is `providedIn: 'root'`, and its whole surface is one method:

```ts
call<T>(module: string, method: string, args?: unknown): Promise<T>
```

For code that is not in an injection context there is `callNative`, with the
same signature and the same implementation.

## What crosses

JSON, in both directions, and nothing else. The arguments are serialised on the
way out and the answer is parsed on the way back.

| Survives | Does not |
|---|---|
| `null`, booleans, numbers, strings | `undefined` in a field, `Date`, `Map`, `Set` |
| arrays, plain objects | functions, symbols, `BigInt`, class instances |

`undefined` as the whole argument becomes `null`. There is no binary channel for
module calls — the command buffer between the core and the hosts is a separate
mechanism and is not exposed here.

## What happens when it goes wrong

Every failure rejects the promise with an `Error`. None of them resolves with
`undefined` and none of them hangs. The message names what was asked for:

| Situation | The rejection says |
|---|---|
| No module by that name is registered | that there is no native module with that name, quoting it |
| The module has no such method | which module, and which method it was asked for |
| The arguments did not deserialise | the module, the method, and the serialiser's complaint |
| The module was dropped without answering | that the module did not answer |
| The answer came back unparseable | that, with the payload |

The last two matter more than they look. A native module hands back a
one-shot responder; if it is dropped without being used — an early return, a
panic, a callback that never fires — the drop itself rejects. The only way to
leave a promise hanging is for a **plugin** to store its call object and never
touch it again, and that is a bug in the plugin, not a state the framework can
reach on its own.

## How a call travels

```text
  JS                     engine thread                    UI thread
  ──────────────────     ─────────────────────────        ──────────────
  call() → Promise
       │  JSON.stringify
       ▼
  invoke ──────────────▶ ModuleRegistry::invoke
       │                     │ finds by name
       │  returns a call id  ▼
       │                 NativeModule::call
       │                     │                  built-in: answers here
       │                     └─ plugin: enqueue ──────▶ registry, next frame
       │                                                     │
       │                 outbox ◀────────────────────────────┘
       ▼                     │ drained at the top of each frame
  promise settled ◀──────────┘
```

The call id comes back synchronously; the *answer* does not. The engine never
blocks. Draining the outbox happens **before** microtasks are drained in each
frame, so a module that answers immediately settles its promise in the same
frame the call was made — but nothing requires it to: a module can hold the
responder across any number of frames, and answer from another thread.

## Writing one, in Rust

The trait is two methods:

```rust
pub trait NativeModule {
    fn name(&self) -> &'static str;
    fn call(&mut self, method: &str, args: Value, respond: Responder);
}
```

`Responder` has `resolve(value)` and `reject(message)`, consumes itself, and
rejects on drop. In practice you do not implement the trait by hand: a macro
takes a list of methods with typed arguments and return values and generates the
dispatch, the deserialisation, the serialisation and the unknown-method
rejection.

Modules are registered on the runtime **before the app is evaluated**, from
whichever host is building it. The registry lives on the engine thread and is
per-runtime.

## What ships built in

One module: `device`.

```ts
import { Device } from '@angular-native/platform'

const info = await inject(Device).info()
```

`info()` takes no arguments and answers with:

| Field | Type | Where it comes from |
|---|---|---|
| `platform` | `NativePlatform` | Compiled in on Apple, hard-coded on Android. |
| `systemVersion` | `string` | `UIDevice.systemVersion` / `Build.VERSION.RELEASE`. |
| `model` | `string` | `UIDevice.model` / `Build.MODEL`. |
| `scale` | `number` | The screen's scale factor. |
| `locale` | `string` | The current locale identifier. |

On the Apple side every value is captured **once**, on the main thread, before
the worker starts, and `info()` clones the cache. On visionOS `scale` is
deliberately `0` rather than a plausible-looking `2`: a window in a headset has
no screen scale, and inventing one would be worse than saying so.

There is no filesystem, network, notifications, haptics, storage or permissions
module in the core. Anything else is a [plugin](/extending/plugins/).

## Which platforms register anything

This is the part that surprises people.

| Platform | `device` | Plugins |
|---|---|---|
| iOS · iPadOS | ✓ | ✓ |
| tvOS | ✓ | ✓ |
| visionOS | ✓ | ✓ |
| Android | ✓ | ✓ |
| Wear OS | ✓ | ✓ |
| **macOS** | — | — |
| **watchOS** | — | — |

`an-macos` and `an-watch` register **no modules at all**. `Device.info()` on
either rejects with "there is no native module called `device`", and there is no
fallback. Both also refuse at build time to package a plugin, so at least the
failure arrives before you ship.

## `NativePlatform`, and the three values nothing produces

```ts
type NativePlatform =
  | 'ios' | 'tvos' | 'visionos' | 'macos' | 'watchos' | 'android' | 'wearos'
```

Seven declared. Four are actually produced at runtime — `ios`, `tvos`,
`visionos` and `android` — and three are not:

- **`macos` and `watchos`** because neither host registers `device` at all.
- **`wearos`** because Wear OS is the Android host, and the Android side
  hard-codes `"android"`. A Wear app cannot tell it is on a watch through this
  module. If you need to know, ask about the screen instead — the round inset
  arrives through `an-safe-area`.

The headless test runner produces a fifth value, `headless`, which is not in the
union at all: a `switch` written against the type has no branch for it.

Do not write a feature check as a platform check. Ask for the thing, and handle
the rejection.

## Routing is not a module

`NativePlatformLocation` looks like it might be one and is not: it is a
pure-JavaScript `PlatformLocation` over an in-memory stack, so Angular's router
works with no address bar and no History API. It never calls native code.

```ts
import { NATIVE_LOCATION_PROVIDERS } from '@angular-native/platform'

bootstrapNativeApplication(App, {
  providers: [provideRouter(routes), NATIVE_LOCATION_PROVIDERS]
})
```

Two members beyond `PlatformLocation` matter, and are why the concrete class is
provided as well as the token: `canGoBack`, which is what the iOS back gesture
and the Android back button consult, and `historyIndex`, which is what gives a
stack transition its direction — comparing the index before and after says
whether you went forward or back.

The whole stack survives a hot reload. Being thrown back to the app's first
screen on every save is the first thing that grates.
