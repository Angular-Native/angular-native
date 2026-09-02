---
title: angular-native
description: React Native, but for Angular, with a Rust core.
---

Angular apps that draw real native views — `UIView` on iOS, `android.view.View`
on Android, SwiftUI on the watch, `NSView` on the Mac. No WebView anywhere.

The tree, the flexbox layout and the diff into mount operations live in a Rust
core that knows nothing about any platform; each platform is a host that
applies what the core decides.

## Where to start

- **Start here** — what it is, and running it on a project of your own.
- **Platforms** — what each one paints, what it does not, and why.
- **Extending it** — plugins: native methods written outside this repo.
