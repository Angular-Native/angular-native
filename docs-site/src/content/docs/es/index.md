---
title: angular-native
description: React Native, pero para Angular, con el núcleo en Rust.
---

Apps de Angular que pintan vistas nativas de verdad — `UIView` en iOS,
`android.view.View` en Android, SwiftUI en el reloj, `NSView` en el Mac. Sin
WebView por ninguna parte.

El árbol, el layout flexbox y el diff a operaciones de montaje viven en un
núcleo de Rust que no sabe de ninguna plataforma; cada plataforma es un host
que aplica lo que el núcleo decide.

## Por dónde empezar

- **Empezar** — qué es esto, y cómo correrlo en un proyecto tuyo.
- **Plataformas** — qué pinta cada una, qué no, y por qué.
- **Extenderlo** — plugins: métodos nativos escritos fuera de este repo.
