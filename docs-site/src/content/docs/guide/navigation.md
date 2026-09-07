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

## What is not there

**No `#` fragments.** There is no address bar to put one in, so
`onHashChange` returns a no-op unsubscriber and `hash` is whatever the URL
string itself carried.

**No deep links yet.** A URL that arrives from outside the app — a custom scheme,
a universal link — is not wired to the router. The stack is in-memory and starts
at `/`.

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
