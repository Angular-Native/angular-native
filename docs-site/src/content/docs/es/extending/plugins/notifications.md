---
title: "@angular-native/plugin-notifications"
description: "Notificaciones locales, y el token para las remotas — UNUserNotificationCenter y NotificationManager, como plugin de angular-native."
sidebar:
  label: notifications
  order: 8
---

Notificaciones que la app programa por su cuenta. `UNUserNotificationCenter` en
las plataformas de Apple y, en Android, `NotificationManager` con un
`AlarmManager` detrás — un proceso al que han matado no puede publicar nada, así
que una notificación programada es del sistema y no de la app.

El toque que **lanzó** la app es el que no conviene perder, y es el más difícil:
el sistema lo entrega antes de que exista JS. Nada nativo puede preguntar si hay
alguien escuchando todavía, así que los toques se retienen hasta que JS se
suscribe por primera vez, y a partir de ahí se emiten en vivo.

`remoteToken()` rechaza y dice por qué. APNs necesita que el shell se registre y
entregue el token; FCM necesita un proyecto de Firebase que este shell no
incluye.

```bash
npm install @angular-native/plugin-notifications
```

## Dónde funciona

| iOS · iPadOS | macOS | watchOS | Android |
|:--:|:--:|:--:|:--:|
| ✓ | ✓ | ✓ | ✓ |

## La API

| Método | Devuelve | |
|---|---|---|
| `permission()` | `Promise<NotificationPermission>` |  |
| `request()` | `Promise<NotificationPermission>` | Pregunta, si hay algo que preguntar, y espera la respuesta. |
| `schedule(notification: LocalNotification)` | `Promise<void>` |  |
| `cancel(id: string)` | `Promise<void>` |  |
| `pending()` | `Promise<string[]>` |  |
| `clearDelivered()` | `Promise<void>` |  |
| `remoteToken()` | `Promise<string>` | El token en el que este dispositivo es alcanzable, para notificaciones remotas. |

Cada método es una llamada que cruza el puente, así que todos devuelven una
promesa, y todo fallo es un rechazo que nombra qué se pidió. Mira
[Módulos nativos](/es/reference/native-modules/) para saber qué cruza y qué
aspecto tiene un rechazo.
