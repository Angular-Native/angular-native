---
title: "@angular-native/plugin-preferences"
description: "Valores pequeños que sobreviven al cierre de la app — UserDefaults y SharedPreferences, como plugin de angular-native."
sidebar:
  label: preferences
  order: 2
---

Valores pequeños que sobreviven al cierre de la app: `UserDefaults` en las
plataformas de Apple y `SharedPreferences` en Android — el almacén que todas
ellas ya tienen para los ajustes, incluido en la copia de seguridad del
dispositivo y legible antes del primer fotograma.

**Solo cadenas, a propósito.** Ambos almacenes pueden guardar números, booleanos,
fechas y arrays, y los conjuntos que pueden guardar no son el mismo conjunto.
Una clave escrita como número en una plataforma y leída como cadena en la otra
es un fallo que solo aparece en la plataforma que nadie probó, así que el cable
lleva un solo tipo y todo lo estructurado pasa por `JSON.stringify`.

**Esto no es almacenamiento seguro.** Un fichero de preferencias es texto plano
en un dispositivo que alguien puede rootear. Los tokens y las claves van en el
plugin de llavero.

```bash
npm install @angular-native/plugin-preferences
```

## Dónde funciona

| iOS · iPadOS | macOS | watchOS | Android |
|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ |

## La API

| Método | Devuelve | |
|---|---|---|
| `get(key: string)` | `Promise<string \| null>` |  |
| `set(key: string, value: string)` | `Promise<void>` |  |
| `remove(key: string)` | `Promise<void>` |  |
| `keys()` | `Promise<string[]>` | Todas las claves que esta app ha escrito, sin orden concreto. |
| `clear()` | `Promise<void>` |  |

Cada método es una llamada que cruza el puente, así que todos devuelven una
promesa, y todo fallo es un rechazo que nombra qué se pidió. Mira
[Módulos nativos](/es/reference/native-modules/) para saber qué cruza y qué
aspecto tiene un rechazo.
