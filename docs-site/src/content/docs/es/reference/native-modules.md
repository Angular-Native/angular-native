---
title: Módulos nativos
description: Cómo llama JavaScript a código nativo, qué cruza la frontera, qué viene integrado — y qué plataformas no registran absolutamente nada.
sidebar:
  order: 4
---

Una primitiva es una vista. Un **módulo nativo** es un método: JS pide algo, el
código nativo contesta, y la respuesta vuelve como una promesa. Es el mismo
mecanismo que usa un [plugin](/es/extending/plugins/) — un plugin es un módulo
nativo registrado desde fuera de este repositorio.

## Llamar a uno

```ts
import { inject } from '@angular/core'
import { NativeModules } from '@angular-native/platform'

export class Battery {
  private readonly modules = inject(NativeModules)

  level(): Promise<number> {
    return this.modules.call<number>('battery', 'level')
  }
}
```

`NativeModules` es `providedIn: 'root'`, y toda su superficie es un método:

```ts
call<T>(module: string, method: string, args?: unknown): Promise<T>
```

Para código que no está en un contexto de inyección existe `callNative`, con la
misma firma y la misma implementación.

## Qué cruza

JSON, en las dos direcciones, y nada más. Los argumentos se serializan a la ida y
la respuesta se parsea a la vuelta.

| Sobrevive | No sobrevive |
|---|---|
| `null`, booleanos, números, cadenas | `undefined` en un campo, `Date`, `Map`, `Set` |
| arrays, objetos planos | funciones, símbolos, `BigInt`, instancias de clase |

`undefined` como argumento entero se convierte en `null`. No hay canal binario
para las llamadas a módulos: el búfer de comandos entre el núcleo y los hosts es
un mecanismo aparte y no se expone aquí.

## Eventos: lo que un módulo dice sin que le pregunten

Una llamada tiene exactamente una respuesta, y por eso `call` devuelve una
promesa. Una posición mientras caminas, una notificación que se pulsa, los
mensajes de un socket: esos no tienen ninguna o tienen mil, y una promesa no
puede llevarlos. Así que un módulo tiene un segundo camino.

```ts
const stop = modules.on<Position>('geolocation', 'position', (donde) => {
  this.aqui.set(donde)
})

inject(DestroyRef).onDestroy(stop)
```

`on` devuelve el desuscriptor y nada más. Varios manejadores pueden escuchar el
mismo evento y cada uno recibe el suyo: un módulo no tiene ni idea de quién
escucha, y que uno se desuscriba no puede dejar sordo a otro.

**Una suscripción que no se para nunca mantiene vivo el manejador, y con él todo
lo que capture, mientras viva la app.** Y peor: del lado nativo suele mantener
algo encendido — un gestor de localización, un socket. Párala.

### Cuándo llegan

Al principio de un frame, en el orden en que se emitieron, y **después** de las
respuestas de ese frame. Ese orden es deliberado: un `start()` que resuelve y
emite de inmediato tiene que asentarse antes de que aterrice su primer evento, o
el evento llega antes que el código que se suscribe.

Un manejador que lanza se reporta y los demás siguen corriendo. El bug de un
componente no es motivo para que todos los demás se pierdan el evento.

### Emitir uno, en Rust

Al módulo se le entrega un `Emitter` al registrarse, y se lo queda:

```rust
impl NativeModule for Ticker {
    fn connect(&mut self, emitter: Emitter) {
        self.emitter = Some(emitter);
    }
    …
}
```

El nombre va ligado al emisor en vez de pasarse a `emit`, así que un módulo no
puede emitir en nombre de otro. Es `Send` y se puede clonar: emitir desde un hilo
de fondo —un callback de localización, una descarga— es el caso normal, no la
excepción.

### Emitir uno desde un plugin

Swift y Java tienen lo mismo bajo un nombre propio:

```swift
AnEvents.emit("geolocation", "position", ["latitude": fix.coordinate.latitude])
```

```java
AnEvents.emit("geolocation", "position", donde);
```

Los dos se pueden llamar desde cualquier hilo. Emitir bajo un nombre que nadie ha
registrado se registra en el log en vez de descartarse en silencio: es una errata
en un plugin, y las erratas que desaparecen son las caras.

## Qué pasa cuando sale mal

Cada fallo rechaza la promesa con un `Error`. Ninguno resuelve con `undefined` y
ninguno se queda colgado. El mensaje nombra lo que se pidió:

| Situación | El rechazo dice |
|---|---|
| No hay ningún módulo registrado con ese nombre | que no hay módulo nativo con ese nombre, citándolo |
| El módulo no tiene ese método | qué módulo, y qué método se le pidió |
| Los argumentos no se deserializaron | el módulo, el método y la queja del serializador |
| El módulo se soltó sin contestar | que el módulo no contestó |
| La respuesta volvió imparseable | eso mismo, con la carga |

Los dos últimos importan más de lo que parece. Un módulo nativo recibe un
respondedor de un solo uso; si se suelta sin usarlo —un return temprano, un
panic, un callback que no salta nunca— el propio soltarlo rechaza. La única forma
de dejar una promesa colgada es que un **plugin** guarde su objeto de llamada y
no lo vuelva a tocar, y eso es un bug del plugin, no un estado al que el framework
pueda llegar por su cuenta.

## Cómo viaja una llamada

```text
  JS                     hilo del motor                   hilo de UI
  ──────────────────     ─────────────────────────        ──────────────
  call() → Promise
       │  JSON.stringify
       ▼
  invoke ──────────────▶ ModuleRegistry::invoke
       │                     │ lo busca por nombre
       │  devuelve un id     ▼
       │                 NativeModule::call
       │                     │                  integrado: contesta aquí
       │                     └─ plugin: encola ───────▶ registro, frame siguiente
       │                                                     │
       │                 buzón ◀─────────────────────────────┘
       ▼                     │ se vacía al principio de cada frame
  promesa resuelta ◀─────────┘
```

El id de la llamada vuelve de forma síncrona; la *respuesta* no. El motor no se
bloquea nunca. El buzón se vacía **antes** que las microtareas en cada frame, así
que un módulo que contesta de inmediato resuelve su promesa en el mismo frame en
que se hizo la llamada — pero nada le obliga: un módulo puede retener el
respondedor durante los frames que quiera, y contestar desde otro hilo.

## Escribir uno, en Rust

El trait son dos métodos:

```rust
pub trait NativeModule {
    fn name(&self) -> &'static str;
    fn call(&mut self, method: &str, args: Value, respond: Responder);
}
```

`Responder` tiene `resolve(value)` y `reject(message)`, se consume a sí mismo, y
rechaza al soltarse. En la práctica el trait no se implementa a mano: una macro
coge una lista de métodos con argumentos y valores de retorno tipados y genera el
despacho, la deserialización, la serialización y el rechazo por método
desconocido.

Los módulos se registran en el runtime **antes de que se evalúe la app**, desde
el host que la esté construyendo. El registro vive en el hilo del motor y es por
runtime.

## Qué viene integrado

Un módulo: `device`.

```ts
import { Device } from '@angular-native/platform'

const info = await inject(Device).info()
```

`info()` no toma argumentos y contesta con:

| Campo | Tipo | De dónde sale |
|---|---|---|
| `platform` | `NativePlatform` | Compilado en Apple, fijo en Android. |
| `systemVersion` | `string` | `UIDevice.systemVersion` / `Build.VERSION.RELEASE`. |
| `model` | `string` | `UIDevice.model` / `Build.MODEL`. |
| `scale` | `number` | El factor de escala de la pantalla. |
| `locale` | `string` | El identificador de la configuración regional actual. |

En el lado de Apple cada valor se captura **una vez**, en el hilo principal,
antes de que arranque el worker, e `info()` clona la caché. En visionOS `scale`
es deliberadamente `0` y no un `2` con buena pinta: una ventana en un visor no
tiene escala de pantalla, e inventarse una sería peor que decirlo.

En el núcleo no hay módulo de sistema de ficheros, de red, de notificaciones, de
háptico, de almacenamiento ni de permisos. Cualquier otra cosa es un
[plugin](/es/extending/plugins/).

## Qué plataformas registran algo

Esta es la parte que sorprende.

| Plataforma | `device` | Plugins |
|---|---|---|
| iOS · iPadOS | ✓ | ✓ |
| tvOS | ✓ | ✓ |
| visionOS | ✓ | ✓ |
| Android | ✓ | ✓ |
| Wear OS | ✓ | ✓ |
| **macOS** | — | — |
| **watchOS** | — | — |

`an-macos` y `an-watch` **no registran ningún módulo**. `Device.info()` en
cualquiera de los dos rechaza con «no hay módulo nativo llamado `device`», y no
hay recambio. Los dos se niegan además en tiempo de compilación a empaquetar un
plugin, así que al menos el fallo llega antes de publicar.

## `NativePlatform`, y los tres valores que no produce nada

```ts
type NativePlatform =
  | 'ios' | 'tvos' | 'visionos' | 'macos' | 'watchos' | 'android' | 'wearos'
```

Siete declarados. Cuatro se producen de verdad en ejecución —`ios`, `tvos`,
`visionos` y `android`— y tres no:

- **`macos` y `watchos`** porque ninguno de los dos hosts registra `device`
  siquiera.
- **`wearos`** porque Wear OS es el host de Android, y el lado de Android tiene
  fijo `"android"`. Una app de Wear no puede saber que está en un reloj por este
  módulo. Si necesitas saberlo, pregunta por la pantalla: el margen redondo llega
  por `an-safe-area`.

El ejecutor de tests headless produce un quinto valor, `headless`, que no está en
la unión: un `switch` escrito contra el tipo no tiene rama para él.

No escribas una comprobación de funcionalidad como una comprobación de
plataforma. Pide la cosa, y gestiona el rechazo.

## El enrutado no es un módulo

`NativePlatformLocation` parece que podría serlo y no lo es: es un
`PlatformLocation` de JavaScript puro sobre una pila en memoria, para que el
router de Angular funcione sin barra de direcciones y sin History API. No llama a
código nativo nunca.

```ts
import { NATIVE_LOCATION_PROVIDERS } from '@angular-native/platform'

bootstrapNativeApplication(App, {
  providers: [provideRouter(routes), NATIVE_LOCATION_PROVIDERS]
})
```

Dos miembros más allá de `PlatformLocation` importan, y son por lo que se provee
la clase concreta además del token: `canGoBack`, que es lo que consultan el gesto
de volver de iOS y el botón de Android, e `historyIndex`, que es lo que le da su
dirección a una transición de pila — comparar el índice antes y después dice si
fuiste hacia delante o hacia atrás.

La pila entera sobrevive a una recarga en caliente. Que te devuelvan a la primera
pantalla de la app en cada guardado es lo primero que raspa.
