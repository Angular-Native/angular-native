---
title: Navigation and the router
description: Angular's router with no address bar — an in-memory history, a native stack that keeps the screen behind alive, and where the back gesture and the Android button come in.
sidebar:
  order: 6
---

Angular's router works here unchanged: the same `provideRouter`, the same route
parameters, the same guards and resolvers, the same `withComponentInputBinding`.
What is different is underneath it. There is no address bar and no browser
history, so the router is given a stack of its own, and the transition between
screens is a real native one rather than a swap of the DOM.

```ts
import { provideRouter, withComponentInputBinding } from '@angular/router'
import {
  bootstrapNativeApplication,
  NATIVE_LOCATION_PROVIDERS,
  NATIVE_STACK_PROVIDERS
} from '@angular-native/platform'

bootstrapNativeApplication(AppComponent, {
  providers: [
    ...NATIVE_LOCATION_PROVIDERS,
    ...NATIVE_STACK_PROVIDERS,
    provideRouter(
      [
        { path: '', component: HomePage },
        { path: 'ship/:id', component: DetailPage }
      ],
      withComponentInputBinding()
    )
  ]
})
```

And in the root template, where a `<router-outlet>` would go:

```html
<an-native-stack />
```

## The two provider sets

They are separate because they answer separate questions, and one of them is
optional.

**`NATIVE_LOCATION_PROVIDERS`** is what makes the router run at all. Angular's
router is written against the browser's History API; here it gets
`NativePlatformLocation`, a `PlatformLocation` over an in-memory stack. Routes
are still URLs — `/ship/42` is a URL and `:id` still binds — they simply exist
nowhere but inside the app.

**`NATIVE_STACK_PROVIDERS`** is what makes going back instant. It installs a
`RouteReuseStrategy` that keeps the screens below the top of the stack alive.

Without it, the router does what it does on the web: destroys the component you
are leaving and builds it again from scratch when you come back. On a phone that
is the wrong default and it is very visible — the scroll position is gone, so is
what was typed into the form, so is anything the component was holding. It is
Angular's own mechanism for exactly this, and it is the one Ionic uses too.

The strategy also prunes. When a navigation goes **backwards**, what was stored
for the screen being abandoned is thrown away: it will not be reached down that
road again, and storing without pruning is a leak with a nice name.

## `<an-native-stack>`

It goes where a `<router-outlet>` would and does the same job, with three
differences:

- the arriving screen slides in and the one being left slides out behind it,
- the one being left stays alive, so going back is instant,
- iOS's edge-swipe gesture and Android's back button both navigate backwards.

Inside it is an `an-stack-view` — the native primitive — wrapping an ordinary
`<router-outlet>`. There is no magic in the component; what it contributes is
the `[transition]` direction and the `(back)` wiring.

## Forwards and backwards

The router says *where* you went. It does not say whether that was forwards or
backwards, and both the animation's direction and the decision to keep or throw
away a screen depend on knowing.

What tells them apart is the position in the history stack.
`NativePlatformLocation` exposes `historyIndex`, and `NavigationDirection`
compares it across `NavigationEnd` events:

```ts
readonly direction = inject(NavigationDirection).direction  // 'push' | 'pop' | 'none'
```

That signal is public, so a component can lean on it for a transition of its
own.

:::note[Why the reuse strategy reads the index directly]
`NativeStackReuseStrategy` does not inject `NavigationDirection`, even though
that is where the direction lives. `NavigationDirection` injects the `Router`,
and the `Router` injects the reuse strategy — a cycle. Reading the index breaks
it, and the index is where the direction comes from anyway.
:::

## Where the back gesture comes in

Both platforms funnel into the same call. iOS's `UIScreenEdgePanGestureRecognizer`
on the left edge and Android's back button — the button, the gesture, whichever
the user has — arrive at `an-stack-view`'s `(back)` output, and
`<an-native-stack>` answers it with `Location.back()`.

From the router's point of view nothing special happened: a popstate came in,
exactly as it would in a browser. That is why guards keep working across a
system back gesture rather than being bypassed by it.

`canGoBack` on `NativePlatformLocation` is what the gesture consults before
deciding it has somewhere to go. At the bottom of the stack there is no screen
behind, and the gesture does nothing.

## The history survives a hot reload

Saving a file rebuilds the bundle and evaluates it in a fresh engine. Without
help that would throw you back to the app's first screen every time, which is
the first thing that grates in a development loop.

The stack is saved through `globalHotState` and restored on the other side, so a
save leaves you on the screen you were on, four levels deep, with the route
parameters intact. This costs nothing in a release build: there is no reload, so
the saved value is never read.

The same mechanism is available to your own components — see
[hot refresh in the CLI reference](/reference/cli/#what-a-save-actually-does)
for the `hotState` signal.

## Deep links

A URL from outside the app — another app, a notification, a link in a browser —
becomes a route. Nothing has to be provided for it beyond
`NATIVE_LOCATION_PROVIDERS`, which you already have.

`an add ios` and `an add android` declare a scheme for you, and the scheme is
your bundle identifier. That is not a stylistic choice: a URL scheme is claimed
device-wide, so two apps both claiming `myapp` leave the system deciding between
them, and the identifier is the one string that is already yours alone. So:

```bash
# iOS simulator
xcrun simctl openurl booted 'com.example.myapp://ship/2'

# Android
adb shell am start -a android.intent.action.VIEW -d 'com.example.myapp://ship/2'
```

opens the app on `/ship/2`, cold or already running.

### How a URL becomes a route

A **custom scheme** has no site in it. `myapp://ship/2` looks to a URL parser
like the path `/2` on the host `ship`, and nobody writing that link means it, so
everything after the scheme is the route: `/ship/2`.

A **universal link or App Link** does have a site, and it is not part of the
route: `https://example.com/ship/2` becomes `/ship/2`.

Query strings and fragments come along. Anything that is not a URL at all is
ignored rather than treated as `/` — navigating home because a link was
unreadable is worse than doing nothing.

### Cold start and already running

They are genuinely different cases and both work.

On a **cold start** the system launches the process *because* of the URL, and it
reaches the shell before Angular exists. The shell hands it to the core before
the bundle is evaluated; `NativePlatformLocation` reads it as it is constructed
and puts it in the stack as the *current* entry, not on top of `/`. So the first
screen the app paints is already the right one — no home screen flashing past —
and the back gesture leaves the app, because there is genuinely nothing behind.

**Already running**, the URL arrives as an event and is pushed. The screen the
person was on stays alive underneath, the transition animates forwards, and back
returns to where they were. A link to the route the app is already showing does
nothing.

The handover between the two is deliberate and not a matter of luck: taking the
queue of waiting links is the same act as saying "I am listening now", so a URL
landing in the gap cannot be delivered twice or lost.

### Doing something else with it

If you want to intercept a link rather than let it navigate — checking a token,
mapping a legacy URL onto a new route — subscribe and do it yourself:

```ts
import { onDeepLink, routeFromUrl } from '@angular-native/platform'

const stop = onDeepLink((url) => {
  console.log('opened with', url, '->', routeFromUrl(url))
})
```

`onDeepLink` sees the links that arrive while the app is running.
`routeFromUrl` is the same function the location uses, exported so your mapping
and the built-in one cannot disagree.

### What you still have to write by hand

The custom scheme is wired. A **universal link** or **App Link** — a real
`https://` address that opens your app instead of the browser — cannot be,
because half of it lives on your web server and nothing in a CLI can put it
there.

On **iOS** you need two things beyond what `an add ios` writes:

1. The `com.apple.developer.associated-domains` entitlement, with
   `applinks:example.com`. `an` does not write your entitlements file, so this
   is yours.
2. `https://example.com/.well-known/apple-app-site-association`, served as
   `application/json` with no redirect, naming your Team ID and bundle
   identifier. Apple fetches it; if it cannot, the link silently opens Safari
   instead, which is the single most common reason a universal link "does not
   work".

The shell already handles `continue userActivity`, so once those two are in
place nothing else changes.

On **Android** you add a second `<intent-filter>` to `android/AndroidManifest.xml`
beside the one `an add android` wrote:

```xml
<intent-filter android:autoVerify="true">
    <action android:name="android.intent.action.VIEW" />
    <category android:name="android.intent.category.DEFAULT" />
    <category android:name="android.intent.category.BROWSABLE" />
    <data android:scheme="https" android:host="example.com" />
</intent-filter>
```

and serve `https://example.com/.well-known/assetlinks.json` with your package
name and the SHA-256 fingerprint of the signing certificate. `autoVerify` is
what makes Android open your app without asking; without the file it falls back
to a chooser.

:::note[Only iOS and Android receive a URL]
macOS, tvOS, visionOS, watchOS and Wear OS do not deliver deep links yet. The
core's side is shared — the mailbox and the JS end are in `an-bridge`, not in a
host — so what is missing on those platforms is only the shell handing the URL
over: `application(_:open:urls:)` on AppKit, and the equivalent elsewhere.
:::

## What is not there

**No `#` fragments.** There is no address bar to put one in, so
`onHashChange` returns a no-op unsubscriber and `hash` is whatever the URL
string itself carried.

**`protocol` is `app:` and `hostname` is `localhost`.** They have to answer
something, since `PlatformLocation` declares them, and there is nothing truthful
to answer. Nothing in the router reads them.

## Tabs are not routes

`an-tab-bar` is a control, not a router outlet. It reports `(select)` with an
index and you decide what that means — swapping a signal, or navigating. Making
it own a route stack per tab, the way `UITabBarController` does natively, is not
something the framework does for you.

## The example

```bash
cargo an dev examples/router
```

Two screens, a parameterised route, `withComponentInputBinding`, and the back
gesture. `scripts/check-router.sh` drives it through `headless` and checks that
a navigation mounts the second screen's nodes and that going back brings the
first one's straight back rather than rebuilding them.

The same example is what proves the deep links, with no simulator:

```bash
# cold start: the URL goes in before the bundle is evaluated
AN_OPEN_URL='playground://ship/3' \
  cargo run -p an-bridge --example headless -- build/bundle/router/main.js 3

# already running: it arrives halfway through
AN_OPEN_URL_LATER='playground://ship/2' \
  cargo run -p an-bridge --example headless -- build/bundle/router/main.js 6
```

`scripts/check-deep-links.sh` runs both and checks that the first paints the
detail screen without the list ever appearing, and that the second pushes.
