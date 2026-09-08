---
title: Eventos
description: Cada output que puede escuchar una plantilla, la carga exacta que lleva, cuándo se entrega, y qué plataformas no pueden entregarlo — con la razón que dan cuando te suscribes igualmente.
sidebar:
  order: 7
---

Un output aquí no es un evento del DOM con un cast encima. Cada uno está tipado,
cada uno lleva una carga declarada, y cada uno es un observable **frío**: el
reconocedor o el listener de la plataforma se engancha cuando Angular se
suscribe y se suelta cuando la vista se destruye. Una vista a la que nadie
escucha no cuesta nada.

```ts
import type { NativePanEvent } from '@angular-native/primitives'

onPan(event: NativePanEvent) { … }
```

## Cuándo llegan

Nunca en mitad de un frame. Un evento nativo se encola y se le entrega a
JavaScript al **principio del frame siguiente**, así que todo lo que pasó entre
dos vsyncs se procesa en un turno y produce un commit.

Un arrastre da un `(pan)` por frame, no uno por muestra táctil. Las muestras de
más costarían cada una una pasada de detección de cambios y producirían un árbol
idéntico al que ese frame va a montar de todas formas.

## En todas las primitivas

Viven en la clase base, así que las 25 los tienen.

### Gestos

| Output | Carga |
|---|---|
| `(press)` | `{ x, y }` — puntos, relativos a la vista que lo recibió |
| `(doublePress)` | `{ x, y }` |
| `(longPress)` | `{ x, y }` — una sola vez, cuando el sistema decide que cuenta |
| `(pan)` | `{ x, y, translationX, translationY, velocityX, velocityY, state }` |
| `(pinch)` | `{ scale, velocity, state }` — `scale` es relativa al inicio del gesto |
| `(rotation)` | `{ rotation, velocity, state }` — radianes desde que empezó |
| `(swipeLeft)` `(swipeRight)` `(swipeUp)` `(swipeDown)` | `{ x, y }` |

`state` es `'begin' | 'move' | 'end' | 'cancel'`. `cancel` no es un fallo: el
sistema se ha llevado el gesto porque ha ganado otro. `velocity` va en puntos por
segundo, útil para dejar que algo siga rodando cuando se levanta el dedo.

La traslación se mide desde donde empezó el dedo, no desde el evento anterior.

:::note[`(rotation)`, no `(rotate)`]
`[rotate]` ya es la transformación y una clase no puede tener dos miembros con el
mismo nombre. En el cable el evento se sigue llamando `rotate`; lo que cambia es
solo el nombre del output.
:::

La historia entera —reconocedores por plataforma, qué significa `cancel` en la
práctica, y cómo se combinan con `[animate]`— está en
[gestos y animación](/es/guide/gestures-and-animation/).

### Layout y pantalla

| Output | Carga | |
|---|---|---|
| `(layout)` | `{ x, y, width, height }` | El frame resuelto, relativo al padre, cada vez que cambia. |
| `(safeArea)` | `{ top, right, bottom, left }` | Los márgenes que reserva el sistema — **el teclado incluido**. |

`(layout)` lo emite el **núcleo**, no una plataforma: el núcleo es quien calcula
el frame, así que funciona igual en todas partes y nunca se implementó dos veces.

`(safeArea)` cambia al rotar, al entrar en pantalla partida y mientras el teclado
se anima. Ver [el área segura y el teclado](/es/guide/safe-area-and-keyboard/).

### Foco y puntero

| Output | Carga | |
|---|---|---|
| `(focus)` | `{ value? }` | `value` solo cuando la vista es un campo de texto. |
| `(blur)` | `{ value? }` | |
| `(hover)` | `{ hovered, x, y }` | Solo escritorio; en el resto se rechaza con un motivo. Un output y no dos, porque un `NSTrackingArea` entrega las dos cosas. |

El foco vivía antes solo en `an-text-input`, porque en un móvil el foco es del
teclado. En una tele es la plataforma entera —el mando recorre las vistas
enfocables y no hay otra forma de resaltar la que está debajo del cursor— así que
se subió a la base.

### El reloj

| Output | Carga | |
|---|---|---|
| `(crown)` | `{ delta, offset, velocity }` | La corona digital, mientras gira. |
| `(crownIdle)` | — | Ha parado. Sin esto no hay forma de saber cuándo dejar de moverse. |

`delta` es el cambio desde el último aviso, que es casi siempre lo que se quiere;
SwiftUI solo entrega el acumulado, y `offset` es ese acumulado desde que la vista
cogió el foco.

Está en la clase base y no en un control porque la corona va a **la vista que
tenga el foco**, sea cual sea — el equivalente a girar la rueda del ratón encima
de algo.

## Por primitiva

| Primitiva | Output | Carga |
|---|---|---|
| `an-switch` | `(onChange)` | `boolean` |
| `an-slider` | `(valueChange)` | `number` |
| `an-stepper` | `(change)` | `{ value: number }` |
| `an-segmented-control` | `(change)` | `{ index: number }` |
| `an-select` | `(change)` | `{ index: number }` |
| `an-date-picker` | `(change)` | `{ value: number }` |
| `an-tab-bar` | `(select)` | `number` — el índice |
| `an-text-input` | `(valueChange)` | `string` |
| | `(submit)` | `string` — la tecla de retorno |
| `an-textarea` | `(change)` | `{ value: string }` |
| `an-search-bar` | `(input)` | `{ value: string }` |
| | `(submit)` | `{ value: string }` |
| `an-scroll-view` | `(scroll)` | `{ x, y }` |
| | `(refresh)` | — tirar para recargar |
| `an-image` | `(load)` | `{ width, height }` — el tamaño intrínseco de la imagen |
| `an-stack-view` | `(back)` | — el gesto del borde, o el botón de Android |
| `an-navigation-bar` | `(back)` | — el botón de volver |
| `an-modal` | `(dismiss)` | — cerrado por el usuario y no por la prop |
| `an-alert` | `(select)` | `number` — qué botón |

Seis de estos entregan el valor en vez de un objeto —`(onChange)`,
`(valueChange)` tanto en el slider como en el campo de texto, y `(select)` en la
barra de pestañas y en la alerta— porque no había nada más en la carga que le
hiciera compañía y un `$event.value` en cada uno era ruido. El resto conserva el
objeto, para que añadir un segundo campo más adelante no rompa nada.

## Lo que una plataforma no puede entregar

Un evento que una plataforma no puede dar **avisa al suscribirse**, y el aviso
lleva la razón. No se queda callado, y la razón es casi siempre del SDK y no una
decisión tomada aquí.

### tvOS

| Evento | Por qué no |
|---|---|
| `(pinch)`, `(rotation)` | La superficie del mando es de un solo toque, y `UIPinchGestureRecognizer` y `UIRotationGestureRecognizer` no están en el SDK de tvOS. |
| `(refresh)` | `UIRefreshControl` no está en el SDK, y de una tele no se tira. |

`(pan)` y los cuatro swipes sí funcionan: la superficie reporta un arrastre.
`(press)` llega del botón central, así que una vista que no puede coger el foco
no se puede pulsar nunca.

### iOS, iPadOS y visionOS

| Evento | Por qué no |
|---|---|
| `(hover)` | El puntero es del escritorio. Mira más abajo. |
| `(crown)`, `(crownIdle)` | La corona digital es del Apple Watch; aquí no hay rueda que girar. |
| `(back)` en `an-stack-view` | Solo en visionOS: `UIScreenEdgePanGestureRecognizer` no está en ese SDK y la ventana no tiene borde del que tirar. |

Cualquier otra cosa que llegue al host sin nada a lo que engancharla se contesta
igual, nombrando el primitivo: `(scroll)` en un `an-view`, `(change)` en un
`an-textarea` —que UIKit informa por un `UITextViewDelegate` que este host no
instala, y que el Mac rechaza por el mismo motivo—.

### Android y Wear OS

| Evento | Por qué no |
|---|---|
| `(hover)` | El puntero es del escritorio. Android sí manda eventos de hover bajo un ratón o un stylus, pero `[cursor]` aquí no significa nada. Mira más abajo. |
| `(crown)`, `(crownIdle)` | Solo en un teléfono: no hay rueda. En Wear OS llegan las dos. |

Como en los hosts de Apple, una salida en un primitivo que no la informa se
contesta nombrando el widget que el nodo montó de verdad.

### macOS

`(swipeLeft)` y compañía funcionan, pero **solo en vistas que sean de este
host**. AppKit no tiene `NSSwipeGestureRecognizer`; el gesto llega como
`swipeWithEvent:` por la cadena de respondedores, y eso hay que atenderlo en la
clase. Enganchar un swipe directamente a un control del sistema —un `NSButton`,
un `NSSlider`— avisa al suscribirse y dice que lo pongas en un `an-view` que lo
envuelva.

### watchOS

El único host que no es una jerarquía de vistas, y el de la lista más larga.

| Evento | La razón que da |
|---|---|
| `(pinch)` | `MagnifyGesture` está marcado `@available(watchOS, unavailable)`, y dos dedos no caben en una pantalla de 40 mm. |
| `(rotation)` | `RotateGesture` está marcado `@available(watchOS, unavailable)`. |
| `(back)` | Fuera de un `NavigationStack` el reloj no da arrastre desde el borde, y montar uno metería el layout de SwiftUI dentro del de taffy. |
| `(refresh)` | En un reloj una lista no se recarga tirando de ella: eso se hace con la corona, que ya llega como `(crown)`. |
| `(scroll)` | El `ScrollView` de SwiftUI no publica su desplazamiento en watchOS 11, que es el mínimo de este shell. |
| `(safeArea)` | Una app de reloj ocupa la pantalla entera y el sistema no reserva márgenes que se puedan preguntar. |
| `(focus)`, `(blur)` | Todavía no: en un reloj el foco es el mismo que decide quién tiene la corona, y dos dueños harían que la corona saltara a otro sitio mientras escribes. |

### Un móvil no tiene puntero

`(hover)` es solo de escritorio, y la prop `[cursor]` que va con él, también. Un
dedo no tiene forma y nada sobrevuela antes de tocar.

Las otras dos familias de hosts *podrían* entregarlo a medias. UIKit trae
`UIHoverGestureRecognizer` —un trackpad de iPad, un ratón encendido con
AssistiveTouch, un Apple Pencil sostenido sobre el cristal— y Android manda
`ACTION_HOVER_ENTER` bajo un ratón o un stylus. No se engancha ninguno de los
dos, y la decisión es deliberada: lo que informan es hardware que la mayoría de
estos dispositivos no tiene, así que la salida se dispararía en el iPad de quien
revisa y nunca en el móvil de quien la usa, y `[cursor]`, la otra mitad de la
pareja, no significa nada en ninguno. Una interfaz que solo responde a un
puntero no se puede usar con un dedo. Así que la suscripción se rechaza, una
vez, con ese motivo, en lugar de descartarse sin decir nada.

## El aviso que deliberadamente no se imprime

Angular registra un listener de elemento para **cada** output que aparece en una
plantilla, incluidos los que no son eventos de plataforma en absoluto:
`onChange`, `valueChange`. Avisar de esos sería avisar en cada arranque de algo
que funciona perfectamente, y un aviso que sale siempre es un aviso que nadie
lee.

Así que cada host lleva una lista de los nombres que el framework sí manda
—`support::KNOWN_EVENTS` en el Mac, `family::KNOWN_EVENTS` para las tres
familias de UIKit, `KNOWN_EVENTS` en `AnHost`— y solo avisa del primer tipo:
algo que una plantilla le pidió de verdad a la plataforma y que esta plataforma
no da. `scripts/check-platform-gaps.sh` lee los rechazos de esos hosts y falla
si una página de plataforma no los nombra.
