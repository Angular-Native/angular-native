---
title: Plugins
description: An npm package that ships native code as well as TypeScript — the contract, one written end to end, and what a missing platform does to your build.
sidebar:
  order: 1
---

A plugin is an **npm package that ships native code as well as TypeScript**. It
covers what the framework does not carry and has no business carrying: the
camera, biometrics, in-app purchases, the clipboard. It is written by people
outside this repository, installed with `npm install`, and `an` does the rest.

```text
  @yourcompany/plugin-camera
    package.json      angularNative: { module, ios, android }
    src/              the TypeScript API the app sees
    native/ios/       Swift
    native/android/   Java

  the app
    package.json      "dependencies": { "@yourcompany/plugin-camera": "^1" }
```

That is everything you declare. There is no separate config file, no install
command, nothing to register by hand: when it assembles the `.app` or the APK,
`an` reads the app's dependencies, keeps the ones carrying a manifest, compiles
their sources alongside the shell and generates the registry.

That this is short is not a credit to the design, it is a credit to the terrain:
there is no `.xcodeproj` and no Gradle here. Linking a plugin in Capacitor or
React Native means editing the Xcode project and `settings.gradle` from a
script. Here the build was already "read some paths and hand them to `swiftc`
and `javac`", so a plugin is a few more paths.

:::note[This is native modules, not native views]
JS calls a method and gets a promise. What is **not** in yet is plugins
contributing *views* — a plugin bringing an `<an-camera>` that mounts in the
tree — because that means opening the core's `NodeKind` to names it does not
know at compile time. See [What is missing](#what-is-missing).
:::

## The contract

Four lines:

1. **The manifest** goes in the plugin's `package.json`, under `angularNative`,
   and gives the module's name, where the TypeScript API is, and where each
   platform's sources are.
2. **The TypeScript API** is an ordinary Angular service calling
   `NativeModules.call(module, method, args)` and returning promises.
3. **The native part** is a Swift type and a Java class implementing `AnPlugin`:
   they receive a method and arguments, and answer through their call object.
4. **The app declares it as a dependency** in its `package.json`, and that is the
   end of it: `an` discovers, compiles and registers.

```jsonc
{
  "name": "@angular-native/plugin-clipboard",
  "angularNative": {
    // The name JS invokes it by. Unique within the app; starts lowercase,
    // letters, digits and hyphens. This is the only place it is written:
    // the Swift registry and the Java one receive it generated.
    "module": "clipboard",

    // The .ts exporting the API, relative to the package root. Optional: a
    // plugin published to npm already compiled does not need it.
    "entry": "src/public-api.ts",

    // One section per platform covered. The one that is missing is the one
    // that will fail that platform's build, on purpose.
    "ios": {
      "sources": "native/ios",           // a directory, walked whole
      "register": "AnClipboardPlugin",   // the Swift type

      // Optional. Info.plist keys and entitlements this plugin needs.
      "plist": { "NSFaceIDUsageDescription": "…" },
      "entitlements": { "keychain-access-groups": ["…"] }
    },
    "android": {
      "sources": "native/android",
      "register": "dev.angularnative.plugins.ClipboardPlugin",

      // Optional. AndroidManifest entries.
      "manifest": { "uses-permission": ["…"], "uses-feature": ["…"] }
    }
  }
}
```

The `plist`, `entitlements` and `manifest` sections have a page of their own:
[Permissions a plugin needs](/extending/plugin-permissions/).

The reference plugins are `packages/plugin-clipboard`,
`packages/plugin-biometrics` and `packages/plugin-keychain`; the last two are
also written up in [Biometrics and keychain](/extending/biometrics-and-keychain/).

## Writing one end to end

A haptics plugin — the smallest thing that is still useful.

### 1. The package

```text
packages/plugin-haptics/
  package.json
  src/public-api.ts
  native/ios/AnHapticsPlugin.swift
  native/android/dev/angularnative/plugins/HapticsPlugin.java
```

```jsonc
{
  "name": "@angular-native/plugin-haptics",
  "version": "0.0.1",
  "type": "module",
  "main": "src/public-api.ts",
  "peerDependencies": { "@angular/core": ">=22.0.0" },
  "angularNative": {
    "module": "haptics",
    "entry": "src/public-api.ts",
    "ios": { "sources": "native/ios", "register": "AnHapticsPlugin" },
    "android": {
      "sources": "native/android",
      "register": "dev.angularnative.plugins.HapticsPlugin"
    }
  }
}
```

### 2. The TypeScript API

An Angular service with no logic: the types out and back, and little else.

```ts
import { inject, Injectable } from '@angular/core'
import { NativeModules } from '@angular-native/platform'

/** How hard. The three both platforms define. */
export type HapticStyle = 'light' | 'medium' | 'heavy'

@Injectable({ providedIn: 'root' })
export class Haptics {
  private readonly modules = inject(NativeModules)

  vibrate(style: HapticStyle = 'medium'): Promise<void> {
    return this.modules.call<void>('haptics', 'vibrate', { style })
  }
}
```

The module name — `'haptics'` — has to match `angularNative.module`. It is the
only string repeated in the whole system, and the two copies are two lines
apart.

### 3. iOS

The file is compiled **inside the `.app`, in the same `swiftc` invocation as the
shell**, so it sees `AnPlugin`, `AnPluginCall` and all of UIKit with nothing
imported and no dependency declared.

```swift
import UIKit

final class AnHapticsPlugin: AnPlugin {
    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall) {
        guard method == "vibrate" else {
            // Never in silence: a method that does not exist rejects.
            respond.reject("the haptics plugin has no method \(method)")
            return
        }
        let style: UIImpactFeedbackGenerator.FeedbackStyle
        switch args["style"] as? String {
        case "light": style = .light
        case "heavy": style = .heavy
        default: style = .medium
        }
        UIImpactFeedbackGenerator(style: style).impactOccurred()
        respond.resolve()
    }
}
```

The whole protocol:

```swift
protocol AnPlugin: AnyObject {
    /// Optional. The screen the app hangs off, before the first call: this is
    /// where a `present` comes from for anything that has to show something.
    func attach(_ host: UIViewController)

    func call(_ method: String, _ args: [String: Any], _ respond: AnPluginCall)
}
```

And the answers:

```swift
respond.resolve()               // the method returns nothing
respond.resolve("some text")
respond.resolve(true)
respond.resolve(3.5)
respond.resolve(["url": path]) // a JSON-serialisable object
respond.reject("what went wrong")
```

`args` is whatever JS sent, already decoded; if it did not send an object it
arrives empty.

### 4. Android

The same with `javac`: it joins the shell's invocation, so it sees `AnPlugin`,
`AnPluginCall` and `android.jar` with no extra classpath. **The Android side of
a plugin is Java, not Kotlin** — see [What is missing](#what-is-missing).

```java
package dev.angularnative.plugins;

import android.app.Activity;
import android.os.VibrationEffect;
import android.os.Vibrator;

import dev.angularnative.AnPlugin;
import dev.angularnative.AnPluginCall;

import org.json.JSONObject;

public final class HapticsPlugin implements AnPlugin {

    private Activity host;

    @Override
    public void attach(Activity host) {
        this.host = host;
    }

    @Override
    public void call(String method, JSONObject args, AnPluginCall respond) {
        if (!"vibrate".equals(method)) {
            respond.reject("the haptics plugin has no method " + method);
            return;
        }
        Vibrator vibrator = host == null ? null : host.getSystemService(Vibrator.class);
        if (vibrator == null) {
            respond.reject("this device does not vibrate");
            return;
        }
        int amplitude = "heavy".equals(args.optString("style")) ? 255 : 128;
        vibrator.vibrate(VibrationEffect.createOneShot(20, amplitude));
        respond.resolve();
    }
}
```

`attach(Activity)` has an empty default implementation: anything that does not
need a context does not write it. On iOS it is `attach(UIViewController)` and
works the same way.

### 5. Using it

In the app's `package.json`:

```json
"dependencies": { "@angular-native/plugin-haptics": "0.0.1" }
```

In its `tsconfig.json`, the same path the framework packages already take —
`ngc` compiles the plugin alongside the app, under the same `rootDir`:

```jsonc
"paths": {
  "@angular-native/platform": ["../../packages/platform-native/src/public-api.ts"],
  "@angular-native/primitives": ["../../packages/primitives/src/public-api.ts"],
  "@angular-native/plugin-haptics": ["../../packages/plugin-haptics/src/public-api.ts"]
}
```

And in the component:

```ts
import { Haptics } from '@angular-native/plugin-haptics'

export class AppComponent {
  private readonly haptics = inject(Haptics)

  confirm(): void {
    this.haptics.vibrate('heavy').catch((error: unknown) => console.error(error))
  }
}
```

Then `npm install`, so npm links the package, and:

```bash
an plugins examples/my-app   # is it seen?
an ios examples/my-app       # does it work?
```

## A missing platform shows itself

**It is the most important rule in this system.** A plugin covering only iOS,
put into an APK, cannot end up as a method returning `undefined` and a screen
that does not react. The build stops, before anything compiles, naming the
plugin and the platforms it does cover, and telling you the two ways out: the
plugin adds its Android half, or the app stops depending on it.

You can ask without building:

```bash
an plugins examples/clipboard
# clipboard  (@angular-native/plugin-clipboard)  ios + android  packages/plugin-clipboard

an plugins examples/clipboard --platform android   # exits 0 if covered, errors if not
```

All five hosts load plugins. What differs is what a plugin can *do* on each,
and a plugin that cannot work on one declares so itself: the build then refuses
that combination and quotes the plugin's own sentence, rather than shipping an
app whose calls would be rejected at runtime. See
[Plugins on the Mac and the watch](/extending/plugins-on-the-mac-and-the-watch/).

And at runtime the same idea: a method the plugin does not handle **rejects the
promise naming the method that was asked for**. It does not hang and it does not
resolve with nothing. That is why both implementations above end in a `reject`
on the unknown-method path.

There is one honest asymmetry here. The **Android** registry wraps the plugin's
`call` in a `try`/`catch` and turns anything that escapes into a rejection; the
**iOS** registry does not. A Swift error escaping a plugin on iOS is not turned
into a rejection for you — end every path in `resolve` or `reject` yourself.

The only way to leave a promise hanging is for a plugin to keep its
`AnPluginCall` and call neither. That is a bug in the plugin, and there is no
deadline cutting it short yet.

## How it works inside

Three pieces, none of which knows more about the others than it has to.

### The queue

A native module's `call` runs on the **JS engine thread**; `UIPasteboard` and
`ClipboardManager` want the **UI thread**. So a plugin is not called, it is
queued:

```text
  engine thread                          UI thread
  ──────────────────                     ─────────────────────
  HostPlugin::call
       │ queues with its responder
       ▼
  PluginBridge ──────── take_calls() ──▶ AnPluginRegistry (Swift/Java)
       ▲                                        │
       └────────── resolve(id, json) ◀──────────┘
```

The UI thread collects the calls inside the frame it is already drawing — and it
answers when it can: immediately for the clipboard, three seconds later if there
is a camera behind it. The engine waits for nobody, and an answer arriving
inside the frame is delivered in that same frame.

The mailbox keeps each in-flight call's responder, and if it is destroyed its
`Drop` rejects them rather than leaving them waiting for ever.

On the Rust side there is **one implementation for every plugin**, `HostPlugin`,
and it is a postman. It knows no plugin's method list, and it should not: the
one that rejects a method that does not exist is the implementation, which is
the only thing that knows its own list.

### The registry

`an` generates, on every build, a file linking the manifest's name to the native
type:

```swift
// build/ios/generated/AnGeneratedPlugins.swift — generated, do not edit
enum AnGeneratedPlugins {
    static func install() {
        AnPluginRegistry.register("clipboard", AnClipboardPlugin())
    }
}
```

```java
// build/android/gen-plugins/dev/angularnative/AnGeneratedPlugins.java
public final class AnGeneratedPlugins {
    public static void install() {
        AnPluginRegistry.register("clipboard", new dev.angularnative.plugins.ClipboardPlugin());
    }
}
```

This is why a plugin **does not declare its own name**: declared in two places —
the manifest and the code — they could stop agreeing, and the failure would be a
module that does not exist at runtime. With one, it cannot happen.

The registry is installed **before the runtime is created**: the core builds one
native module per plugin when the engine starts, and anything registered after
that would not get in. If the shell never installed the dispatcher at all, calls
reject saying so — and a plugin the app does not carry rejects saying it is not
in this `.app` or this APK.

### The linking

The CLI's plugin module does everything else:

| Step | What it does |
|---|---|
| Discover | Reads the app's `package.json` `dependencies`, resolves each in `node_modules` — the app's first, then the root's, the way Node does — and keeps the ones carrying `angularNative`. |
| Validate | A unique, well-formed module name, an `entry` that exists, source directories that exist and are not empty. |
| Require | That every one of them covers the platform being built. |
| Compile | Adds their `.swift` to the `swiftc` line and their `.java` to the `javac` line, next to the shell's. |
| Register | Writes `AnGeneratedPlugins`. |
| Merge | Folds their plist keys, entitlements and manifest entries into the app's. |
| Bundle | Adds one esbuild alias per plugin, so the package's import points at the JS `ngc` just emitted. |

That last alias is only needed for plugins carrying their TypeScript API inside
this repository. One published to npm already comes compiled, and esbuild
resolves it through `node_modules` like any other dependency.

## Checking it without a device

`scripts/check-plugins.sh`, which runs inside `check-all.sh`, covers all three
halves: that the plugin is discovered, that a missing platform stops the build,
and that a call goes out and comes back.

The third is tested with the headless runner, which has neither Swift nor Java
but has everything else. `AN_PLUGINS` mounts a module of canned answers per
name:

```bash
an build examples/clipboard
AN_PLUGINS='{"clipboard":{"read":"test text","write":null,"hasText":true}}' \
  cargo run -p an-bridge --example headless -- build/bundle/clipboard/main.js 6
```

```text
-- fake plugin: clipboard
...
Text#15 [20,296 353x19]  "on the clipboard: test text"
```

Without `AN_PLUGINS` the module does not exist, and what you read in the tree is
the rejection rather than a silent gap — which is exactly what is being checked.
A method with no canned answer is rejected too, with the method's name in it.

## What is missing

- **Native views.** A plugin can contribute methods, not primitives. For it to
  bring an `<an-camera>` that mounts in the tree, the core's `NodeKind` would
  have to open up to names it does not know at compile time, and all the hosts
  would have to build a view that is not theirs. That is the big piece.
- **Kotlin.** The Android shell is Java compiled with `javac` against
  `android.jar`; there is no Gradle here, and without Gradle no `kotlinc` comes
  for free. A plugin with `.kt` sources **stops the build** and says so, rather
  than producing an APK with those files silently left out.
- **A deadline for one that does not answer.** A plugin that keeps its call
  object and calls neither `resolve` nor `reject` leaves the promise waiting. It
  would want a per-call timeout — a camera takes minutes, reading the clipboard
  does not — and there is none.
- **Dependencies of dependencies.** Only the app's direct `dependencies` are
  read. A plugin that itself depends on another plugin does not drag the second
  one in.
- **Assets.** Plist keys, entitlements and manifest entries a plugin *can*
  contribute. Files of its own it cannot.
