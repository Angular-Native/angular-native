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

:::note[Mostly methods, and one view]
A plugin is native modules first: JS calls a method and gets a promise. It can
also contribute **one** kind of thing to the tree — a view mounted through
`<an-custom>` — and that hole is deliberately exactly one hole wide. See
[A view a plugin brings](#a-view-a-plugin-brings).
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

## What ships

Nine plugins live in this repository. They are published like any other, and
they are also the worked examples: each one is a real answer to a real platform
question rather than a stub, and where a platform cannot do the thing, the
refusal says why.

| Plugin | What it is | Where it works |
|---|---|:--|
| `plugin-preferences` | `UserDefaults` and `SharedPreferences`, in a store of their own | iOS · Android · macOS · watchOS |
| `plugin-clipboard` | `UIPasteboard` and `ClipboardManager` | iOS · Android · macOS |
| `plugin-keychain` | The keychain and the Android key store | iOS · Android · macOS · watchOS |
| `plugin-biometrics` | Face ID, Touch ID and `BiometricPrompt` | iOS · Android · macOS |
| `plugin-geolocation` | `CLLocationManager` and the platform's `LocationManager`, foreground and background | iOS · Android · macOS |
| `plugin-camera` | The camera and the photo library, presented | iOS · Android · macOS |
| `plugin-notifications` | Local notifications, and the tap that launched the app | iOS · Android · macOS · watchOS |
| `plugin-updater` | New JavaScript without a store release | iOS · Android · macOS |
| `plugin-barcode` | `AVCaptureMetadataOutput`, full screen | iOS · macOS |

Four of them refuse a platform outright, and the reasons are worth reading
because they are the shape of every gap in this project: a watch has no camera
and no pasteboard; a scanner on Android would need Play Services or a decoder
bundled into every app; a background fix on a watch is a different feature with
a different lifecycle rather than the same call on a smaller screen.

`plugin-keychain` and `plugin-biometrics` are written up together in
[Biometrics and keychain](/extending/biometrics-and-keychain/).

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
`AnPluginCall` and `android.jar` with no extra classpath. Java or Kotlin, or
both — see [Kotlin](#kotlin) below.

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

#### Kotlin

A `.kt` beside the `.java`, or instead of it. `an` runs `kotlinc` before `javac`
into the same classes directory, so the two see each other in both directions:
Kotlin calls the shell's Java, and the registry `an` generates —which is Java—
instantiates a Kotlin plugin.

```kotlin
package dev.angularnative.plugins

import android.app.Activity
import dev.angularnative.AnPlugin
import dev.angularnative.AnPluginCall
import org.json.JSONObject

class HapticsPlugin : AnPlugin {
    private var host: Activity? = null

    override fun attach(host: Activity) {
        this.host = host
    }

    override fun call(method: String, args: JSONObject, respond: AnPluginCall) {
        when (method) {
            "buzz" -> respond.resolve()
            else -> respond.reject("the haptics plugin has no method $method")
        }
    }
}
```

The compiler is not part of the Android SDK and not part of the JDK — Gradle
downloads it, and there is no Gradle here — so it is vendored, pinned, and
fetched once:

```bash
python3 scripts/fetch-android-deps.py
```

It runs only when a plugin actually brought a `.kt`: an app whose plugins are
all Java never starts a JVM for it. A build that needs it and cannot find it
**stops**, naming the file, rather than signing an APK with a plugin missing
from it. `AN_KOTLINC` points at the `lib` directory of a distribution you
already have.

The `kotlin-stdlib` the app carries is the one Material already depends on, so a
Kotlin plugin adds nothing to the APK beyond its own classes.

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

## A view a plugin brings

A plugin contributes methods. That was true without exception until recently,
and it is why a barcode scanner could only ever open full screen: a live preview
is a view, and there was no way for one to exist.

There is now exactly one hole, and it is worth understanding why it is shaped
this way. `NodeKind` in the core is a closed enum whose byte codes are frozen in
the protocol — that byte is the thing four files in three languages have to
agree about, and `check-kinds.sh` exists to keep them agreeing. Opening it to
arbitrary names would end that. So instead there is **one** new kind, `Custom`,
and the name travels as a prop: the kind says "ask the host's plugin-view
registry", and `an:view` says which one.

A plugin registers a factory:

```swift
AnPluginViews.register("barcode-preview") { AnBarcodePreview() }
```

```java
AnPluginViews.register("barcode-preview", context -> new BarcodePreview(context));
```

and a template mounts it like any other box:

```html
<an-custom [view]="'barcode-preview'" [style.height]="'260'" [borderRadius]="16" />
```

Three things follow from the design, and all three are load-bearing:

- **It is never measured by its content.** Measuring would mean the core calling
  into a plugin during layout, on the engine thread, and the whole measuring
  path is built the other way round. Give it a size; a view with none comes out
  at zero, which looks like a plugin that does not work.
- **A name nobody registered mounts nothing** and says so once in the log, naming
  the name. Once per name and not once per node: a list of five hundred rows
  with the same missing view is one mistake.
- **Not on watchOS.** That host mirrors the tree into a model SwiftUI redraws
  rather than mounting views, so there is nowhere to put one.

This is not a way to add a primitive. A primitive is a control the framework
mounts on **every** host, with a name every layer agrees about and a check that
keeps them agreeing. A plugin view is one platform's view, mounted where the
template asked, sized by the layout and nothing else — and an app using it is
choosing to be that much less portable, on purpose.

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

Every registry does this the same way now. `call` is `throws` on the Apple
shells and an error that escapes becomes the rejection of that promise, naming
the module and the method, exactly as the Android registry has always done with
a `RuntimeException`. A plugin that does not throw is unaffected: in Swift a
non-throwing method satisfies a throwing requirement, so nothing that compiled
before has to change — what it buys is that a `try` inside a plugin no longer
has to be a `try?` that swallows the reason.

What no registry can catch is a **trap**: a force unwrap of nil, an index past
the end, `fatalError`, a Java `Error`. Those take the process with them and no
`catch` reaches them.

So the only way left to leave a promise hanging is for a plugin to keep its
`AnPluginCall` and call neither. Nothing cuts that short — but it is no longer
invisible: a call that has gone a minute without an answer is named in the log
once, with its module and its method.

It warns and does not reject, deliberately. Cutting the call would mean
deciding, from inside the mailbox, that a plugin waiting on a person has
failed, and nothing there can tell that apart from a plugin that lost its call
object. A camera is open for as long as the person using it takes. The
decision belongs to the plugin, which is the only one that knows which of its
methods wait on people; until it can say so, the honest thing is to make the
wait visible rather than guess at it.

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
| Discover | Reads the app's `package.json` `dependencies`, resolves each in `node_modules` the way Node does — climbing from the package that wants it — and keeps the ones carrying `angularNative`. It walks **through** the plugins it finds, so a plugin that depends on another plugin brings it along; an ordinary library's dependencies are not followed, because nothing behind one can be a plugin this app decided to carry. |
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

- **A plugin view is not a primitive.** `<an-custom>` mounts one, but it is
  never measured by its content, it does not exist on watchOS, and it has no
  props of its own beyond the box the layout gives it. A plugin cannot add a
  control the framework mounts on every host —
  [the section above](#a-view-a-plugin-brings) says why that stays closed.
- **A deadline for one that does not answer.** A plugin that keeps its call
  object and calls neither `resolve` nor `reject` leaves the promise waiting. It
  would want a per-call timeout — a camera takes minutes, reading the clipboard
  does not — and there is none.
- **Assets.** Plist keys, entitlements and manifest entries a plugin *can*
  contribute. Files of its own it cannot.
