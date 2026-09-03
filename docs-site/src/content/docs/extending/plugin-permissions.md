---
title: Permissions a plugin needs
description: How a plugin declares Info.plist keys, entitlements and AndroidManifest entries, and how `an` merges them.
sidebar:
  order: 2
---

A plugin that talks to the camera, the microphone, the keychain or Face ID does
not only need code. It needs the app it is installed into to *declare* something
— and until now it had no way to say so, which meant whoever installed the
plugin had to know that list by heart.

The worst case is Face ID. Without `NSFaceIDUsageDescription` in the
`Info.plist`, iOS does not warn and does not return an error: it kills the
process the moment the policy is evaluated. From the outside the app simply
disappears.

So a plugin declares what it needs in its own `package.json`, and `an` merges it
when it assembles the `.app` and the APK.

## What a plugin can declare

```jsonc
{
  "angularNative": {
    "module": "biometrics",
    "ios": {
      "sources": "native/ios",
      "register": "AnBiometricsPlugin",

      // Keys added to the app's Info.plist.
      "plist": {
        "NSFaceIDUsageDescription": "To check it is you before showing what is stored."
      },

      // Entitlements linked into the binary. `$(BUNDLE_ID)` is replaced with
      // the app's bundle identifier, which the plugin cannot know.
      "entitlements": {
        "keychain-access-groups": ["$(BUNDLE_ID)"]
      }
    },
    "android": {
      "sources": "native/android",
      "register": "dev.angularnative.plugins.BiometricsPlugin",

      "manifest": {
        "uses-permission": ["android.permission.USE_BIOMETRIC"],
        "uses-feature": { "android.hardware.fingerprint": false }
      }
    }
  }
}
```

Nothing else changes: the app still only declares the plugin as a dependency.

### Accepted values

`plist` and `entitlements` take strings, booleans, numbers and arrays of
strings. A nested dictionary stops the build with a message saying so — merging
one properly is not implemented, and merging it badly would be worse than
refusing. Keys must be top level and must not contain a dot, because the key
travels to `plutil -replace`, which treats a dot as a path separator.

`manifest` takes exactly two sections today:

| Section | Shape | Becomes |
|---|---|---|
| `uses-permission` | array of names | `<uses-permission android:name="…" />` |
| `uses-feature` | name → `required` | `<uses-feature android:name="…" android:required="…" />` |

Anything else stops the build rather than being ignored.

## How the merge works

`an` reads every plugin the app depends on and merges what they ask for. The
project's own `Info.plist` and `AndroidManifest.xml` are **never edited** — they
belong to whoever wrote them. The merged copies are written under `build/`.

Every key that gets added is printed:

```console
$ an ios examples/secrets
==> plugin biometrics (1 Swift source)
==> plugin keychain (1 Swift source)
==> entitlements: keychain-access-groups (from @angular-native/plugin-keychain)
==> Info.plist: NSFaceIDUsageDescription (from @angular-native/plugin-biometrics)
```

### Two plugins asking for the same thing

**Same key, same value** is not a conflict. They are saying the same thing, so
it is written once. This is the common case: the biometrics plugin and the
keychain plugin both need `NSFaceIDUsageDescription`, and both ship the same
sentence.

**Same key, different values** stops the build. There is no honest way to pick
one: taking the first by dependency order or alphabetically would silently
decide which sentence the user reads in a system dialog.

```console
$ an plugins my-app --platform ios
Error: two plugins ask for the key "NSFaceIDUsageDescription" of the Info.plist
with different values:
  · @acme/plugin-face
      "To unlock the app."
  · @other/plugin-vault
      "To open the vault."

Only one can stay, and picking by order would silently decide something that
shows up on screen.
Either the two plugins agree, or the app keeps one of the two.
```

Android permissions cannot clash — a permission has no value, so asking for it
twice is asking for it once. Features can: `android:required` is a value, and
*required* and *optional* are not the same request. That one stops the build
too.

The check runs inside `an plugins <app> --platform ios`, before `swiftc` is
invoked, so a clash surfaces in a second instead of half a minute.

### When the app already declares it

If the app's own `Info.plist` or `AndroidManifest.xml` already declares the key,
the app wins — it is its file, and its author is an unambiguous owner. But not
silently: the line saying whose value was ignored goes to stderr.

```console
==> Info.plist: NSFaceIDUsageDescription is already declared by the app
    ("Unlock Acme"); ignoring @angular-native/plugin-keychain's ("To check it is you…")
```

## iOS entitlements: why they are linked, not signed

Entitlements are what lets an app *ask* the system for something. Without
`keychain-access-groups` or `application-identifier`, `SecItemAdd` answers
`errSecMissingEntitlement` (−34018) — "the client has neither" — because the app
belongs to no keychain group, so there is nowhere to store anything.

For the Simulator the entitlements go **inside the binary**, in a
`__TEXT,__entitlements` section that `an` asks the linker for:

```
-Xlinker -sectcreate -Xlinker __TEXT -Xlinker __entitlements -Xlinker <file>
```

Signing them does not work — not ad hoc, and not with a real Apple Development
identity. `keychain-access-groups` is a restricted entitlement, and macOS
refuses to execute a binary carrying it in its signature without a provisioning
profile to back it. The symptom is that the app stops launching, with a
"request denied by SBMainWorkspace" that never mentions entitlements. Xcode does
exactly the same thing for the Simulator.

`application-identifier` is added by `an`, not by the plugin: a plugin has no
way of knowing which app it will end up in. It is also what gives `$(BUNDLE_ID)`
something to expand to.

## What is still missing

- **Real devices.** `an` installs on the Simulator, and the link-section trick
  is a Simulator thing. A device needs a signing identity and a provisioning
  profile, and neither is wired up.
- **Nested dictionaries in the plist.** `NSAppTransportSecurity` and friends
  cannot be contributed yet. The build says so instead of half-merging them.
- **Assets.** A plugin still cannot ship an icon, a sound or a `.strings` file.
- **Anything but `uses-permission` and `uses-feature`** on Android — no
  `<queries>`, no `<provider>`, no attributes on `<application>`.
