---
title: Plugins on the Mac and the watch
description: The two hosts that used to refuse plugins outright now load them — what changed in the manifest, what a watch genuinely cannot do, and why an entitlement is the difference between an app that runs and one that is killed on launch.
sidebar:
  order: 4
---

Two of the hosts used to turn plugins away. `an macos` stopped the build of any
app that depended on one, and `an watchos` did the same; both said so honestly,
and both were holes. They are closed.

Nothing about [the contract](/extending/plugins/) changed. What changed is that
`angularNative` now has a `macos` and a `watchos` section alongside `ios` and
`android`, that the same registry mechanism runs on all four hosts, and that a
plugin can say **why** it cannot cover a platform instead of merely failing to.

## The manifest

```json
{
  "angularNative": {
    "module": "clipboard",
    "entry": "src/public-api.ts",
    "ios":     { "sources": "native/ios",     "register": "AnClipboardPlugin" },
    "android": { "sources": "native/android", "register": "dev.angularnative.plugins.ClipboardPlugin" },
    "macos":   { "sources": "native/macos",   "register": "AnClipboardPlugin" },
    "watchos": {
      "unsupported": "watchOS has no system pasteboard: UIPasteboard is API_UNAVAILABLE(watchos) and there is nothing standing in for it."
    }
  }
}
```

`macos` and `watchos` take the same keys `ios` takes — `sources`, `register`,
`plist`, `entitlements` — because they are the same kind of thing. What is new
is `unsupported`.

### `unsupported`, and why it is worth a key of its own

A build that stops saying *"plugin-biometrics does not cover watchOS"* tells you
nothing you could not have read off the `package.json` yourself. A build that
stops saying *"a watch has no biometric sensor, and
`LAPolicyDeviceOwnerAuthenticationWithBiometrics` is `API_UNAVAILABLE(watchos)`"*
tells you the thing only the plugin's author knew.

So a platform section can carry `unsupported` instead of `sources`, and the
refusal quotes it:

```
this app cannot be built for watchOS: 1 of its plugins does not cover it
  · @angular-native/plugin-biometrics (module "biometrics") says it cannot: a watch has
    no biometric sensor, and LocalAuthentication says so in the header: …

These are not unfinished halves: they cannot exist on watchOS. The app has to stop
depending on them for this build, or not be built for watchOS.
```

That last paragraph is the other half of the point. A plugin that simply has not
got round to a platform is told to go and write the missing half; a plugin that
has decided is not, because telling somebody to write a pasteboard for a device
that has none is telling them to do something impossible.

`sources` and `unsupported` in the same section is an error, and so is an empty
`unsupported`: the reason is the whole field.

## What the three shipped plugins do

| | macOS | watchOS |
|---|---|---|
| `plugin-clipboard` | `NSPasteboard.general` | **cannot** — `UIPasteboard` is `API_UNAVAILABLE(watchos)` |
| `plugin-biometrics` | `LAContext`, Touch ID, optionally a paired watch | **cannot** — no sensor; the biometrics policy is `API_UNAVAILABLE(watchos)` |
| `plugin-keychain` | Keychain Services, with a caveat below | **works**, minus `requireBiometrics` |

### The clipboard is not the iOS file under an `#if`

`NSPasteboard` is not `UIPasteboard`, and the difference is not cosmetic. A
pasteboard holds several representations of one thing, and `setString` **adds**
one rather than replacing what is there — so writing without `clearContents()`
first leaves the previous flavours in place, and the next app to paste can pick a
different one and get the old text back. `clearContents()` is also what bumps the
change count, which is how every `NSPasteboard` observer on the machine learns
anything happened.

### The watch has no biometry, and the header is what settles it

It is worth being precise, because the obvious answer is nearly right. watchOS
does ship LocalAuthentication, `LAContext` exists there, and watchOS 9 added
`LAPolicyDeviceOwnerAuthenticationWithWristDetection` — a real policy you can
really call, which answers *"this watch is on the wrist it was unlocked on"*.

It is still not biometry. `LAPolicyDeviceOwnerAuthenticationWithBiometrics` is
`API_UNAVAILABLE(watchos)`, and so are `biometryNotAvailable`,
`biometryNotEnrolled` and `biometryLockout`. Returning `success` from wrist
detection under a method called `authenticate` would be a lie about what had been
verified, so the plugin refuses and points at the keychain instead.

### The keychain works on a watch, minus one option

Security.framework is all there: the same `SecItemAdd`, the same Secure Enclave,
the same app-private store, and only one keychain rather than the Mac's two.
Items are `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`, which on a watch is
a much shorter window than the same constant means on a phone left on a desk —
a watch locks itself the moment it comes off the wrist.

`requireBiometrics` comes back `unavailable` with **nothing stored**. watchOS does
have `kSecAccessControlUserPresence`, which there resolves to the passcode, and
using it would have been the easy thing: the call would have succeeded and the
app would have believed its secret was behind a fingerprint. That is the one
mistake nobody can see from the outside.

## Entitlements, which matter far more on the Mac

On iOS one plugin needs an entitlement. On the Mac nearly every one does, because
the App Sandbox denies by default and each capability is asked for by name — the
network, the microphone, a file the user picked, the keychain. They are declared
by the plugin and merged by `an`, exactly as the `Info.plist` keys are.

Two things about the Mac are worth knowing before you spend an afternoon on them.

**A Mac has two keychains.** The old file keychain — `login.keychain-db`, the one
Keychain Access shows — is per user and shared by every app that asks. The
data-protection keychain is the iPhone's: per app, sealed to the Secure Enclave,
and the only one that understands `kSecAttrAccessible` or an access control bound
to Touch ID. Reaching it needs `keychain-access-groups`. `plugin-keychain` finds
out which one it got by writing a throwaway item at startup and reports the answer
through `backing()`, rather than assuming.

**An ad-hoc signature cannot carry every entitlement.** macOS splits them in two.
The `com.apple.security.` ones are restrictions an app puts on itself, and anybody
may sign them. The rest — `keychain-access-groups`, `application-identifier`,
`com.apple.developer.*` — are permissions the system *grants*, and it will not
grant one on the word of a signature that belongs to nobody. It does not refuse
the capability: **it refuses to run the binary**, before its first line, with a
bare `Killed: 9`.

That is character for character the same symptom as a missing
`com.apple.security.cs.allow-jit`, which is what QuickJS dies without, and telling
the two apart from the console alone is not possible. So `an macos` leaves the
profile-backed ones out of an unsigned build and says so:

```
==> warning: keychain-access-groups (asked for by @angular-native/plugin-keychain)
    is left out of this build.
    An ad-hoc signature cannot carry it —macOS wants a provisioning profile behind
    it— and an .app that carries it anyway is killed the instant it launches, with
    a bare `Killed: 9`.
```

`an macos --sign` gives it the real one. Everything else keeps working meanwhile,
and the plugin says what it fell back to.

## What the shells look like

The registry is `an-ios`'s, deliberately: the same mailbox in `an-bridge`, the
same register / dispatch / resolve / reject quartet, the same `pump()` inside the
frame so a plugin touches AppKit or WatchKit on the main thread. Two differences,
both of them real:

- **`attach` takes an `NSViewController` on the Mac**, where iOS gives a
  `UIViewController`. That one line is why a plugin keeps a separate source
  directory per platform.
- **The watch protocol has no `attach` at all.** Its shell is a SwiftUI `App` and
  there is no view controller anywhere. A plugin that needs to put something on
  screen cannot be written against it, and a stand-in that handed over nothing
  would only hide that until run time.

## Checking it

`scripts/check-plugins-hosts.sh` covers all of the above without a device: the
refusals and their wording, the generated registry, the entitlement that has to
be in the signature and the one that must not be, and then a real press on a
running Mac window with `NSPasteboard` written and read back.

The watch's end-to-end run is behind `AN_CHECK_WATCH_APP=1`, because
`aarch64-apple-watchos-sim` is a tier 3 target and building `std` from source
takes minutes. `examples/watch-secrets` does its whole round trip on startup and
writes the verdict in its first line — storing, reading back, deleting, and then
confirming that `requireBiometrics` is refused — because tapping a watch simulator
needs the Mac's own mouse over the Simulator window, and a check does not always
have a desktop to borrow.
