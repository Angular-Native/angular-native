---
title: "@angular-native/plugin-geolocation"
description: "Dónde está el dispositivo — CLLocationManager y el LocationManager de Android, como plugin de angular-native."
sidebar:
  label: geolocation
  order: 6
---

Dónde está el dispositivo: `CLLocationManager` en las plataformas de Apple y el
`LocationManager` propio de Android — **no** el proveedor fusionado de Google,
que vive en los Play Services y produciría una app que no se puede instalar en
un dispositivo sin ellos.

`current()` es una posición. `watch()` es un flujo, y no es `current()` en un
bucle: la plataforma decide cuándo hay una posición nueva que merezca la pena
informar, y un sondeo o se perdería movimientos o mantendría el GPS encendido
para nada.

`watch({ background: true })` sigue funcionando con la app fuera de pantalla. En
Android eso significa un servicio en primer plano y **una notificación que la
persona ve mientras dure** — la plataforma lo exige precisamente para que una
app no pueda seguir a nadie en silencio.

```bash
npm install @angular-native/plugin-geolocation
```

## Dónde funciona

| iOS · iPadOS | macOS | watchOS | Android |
|:--:|:--:|:--:|:--:|
| ✓ | ✓ | — | ✓ |

Donde no funciona, el motivo es del propio plugin y lo dice la compilación, no
la primera llamada en tiempo de ejecución:

**watchOS** — watchOS sí tiene CoreLocation, pero una app de reloj que quiera una posición tiene que declarar un modo de segundo plano y mantener una sesión abierta: eso es otra funcionalidad con otro ciclo de vida, no la misma llamada en una pantalla más pequeña. Una app de reloj que necesite una posición debería pedírsela al teléfono emparejado.

## La API

| Método | Devuelve | |
|---|---|---|
| `permission()` | `Promise<LocationPermission>` |  |
| `request()` | `Promise<LocationPermission>` | Pregunta, si hay algo que preguntar, y espera la respuesta. |
| `current(timeoutMs = 10_000)` | `Promise<Position>` | Una posición. |
| `watch(onFix, options: WatchOptions = {})` | `() => void` | Un flujo de posiciones. Llama a lo que devuelve para pararlo. |

`watch` es la excepción a la regla de abajo: es una suscripción, así que
devuelve la función que la termina y no una promesa. Todos los demás métodos son
llamadas que cruzan el puente, así que devuelven una promesa, y todo fallo es un
rechazo que nombra qué se pidió. Mira
[Módulos nativos](/es/reference/native-modules/) para saber qué cruza y qué
aspecto tiene un rechazo.
