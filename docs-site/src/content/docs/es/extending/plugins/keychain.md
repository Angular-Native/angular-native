---
title: "@angular-native/plugin-keychain"
description: "El llavero y el keystore de Android, como plugin de angular-native."
sidebar:
  label: keychain
  order: 4
---

Secretos donde el sistema guarda los suyos: el llavero en las plataformas de
Apple y, en Android, una clave AES en el almacén de claves — que nunca sale de
él — cifrando un fichero privado de la app. Android no tiene llavero, y esa
diferencia está explicada en
[Biometría y llavero](/es/extending/biometrics-and-keychain/).

```bash
npm install @angular-native/plugin-keychain
```

## Dónde funciona

| iOS · iPadOS | macOS | watchOS | Android |
|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ |

## La API

| Método | Devuelve | |
|---|---|---|
| `set(key: string, value: string, options: KeychainOptions = {})` | `Promise<KeychainWrite>` | Guarda o reemplaza un secreto. |
| `get(key: string, options: KeychainOptions = {})` | `Promise<KeychainRead>` |  |
| `has(key: string)` | `Promise<boolean>` | Si hay algo guardado bajo esa clave, sin leerlo y sin pedir biometría. |
| `remove(key: string)` | `Promise<boolean>` |  |
| `backing()` | `Promise<KeychainBacking>` | Qué guarda los secretos en este dispositivo concreto. |

Cada método es una llamada que cruza el puente, así que todos devuelven una
promesa, y todo fallo es un rechazo que nombra qué se pidió. Mira
[Módulos nativos](/es/reference/native-modules/) para saber qué cruza y qué
aspecto tiene un rechazo.
