---
title: "@angular-native/plugin-biometrics"
description: "Face ID, Touch ID and BiometricPrompt, as an angular-native plugin."
sidebar:
  label: biometrics
  order: 5
---

Face ID, Touch ID and `BiometricPrompt`. The check is the system's, the dialog
is the system's, and what comes back is whether the person passed it — never the
biometric itself, which never leaves the enclave.

```bash
npm install @angular-native/plugin-biometrics
```

## Where it works

| iOS · iPadOS | macOS | watchOS | Android |
|:--:|:--:|:--:|:--:|
| ✓ | ✓ | — | ✓ |

Where it does not, the reason is the plugin's own and the build says it
rather than the first call at run time:

**watchOS** — a watch has no biometric sensor, and LocalAuthentication says so in the header: LAPolicyDeviceOwnerAuthenticationWithBiometrics is API_UNAVAILABLE(watchos), and so are biometryNotAvailable, biometryNotEnrolled and biometryLockout. What watchOS does have is the passcode and, from watchOS 9, LAPolicyDeviceOwnerAuthenticationWithWristDetection, which answers 'this watch is on the wrist it was unlocked on' -- a real check, but not a biometric one, and returning success from it under a method called authenticate would be a lie about what was verified. To protect a secret on a watch, use @angular-native/plugin-keychain: its watchOS half stores through the same Keychain Services and can bind an item to the device passcode.

## The API

| Method | Returns | |
|---|---|---|
| `availability()` | `Promise<BiometricAvailability>` | Whether biometrics can be asked for, without asking for them: it presents no dialog.  |
| `authenticate(request: BiometricRequest)` | `Promise<BiometricResult>` |  |

Every method is a call across the bridge, so every one returns a promise, and
every failure is a rejection that names what was asked for. See
[Native modules](/reference/native-modules/) for what crosses and what a
rejection looks like.
