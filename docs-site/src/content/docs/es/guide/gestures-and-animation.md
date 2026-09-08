---
title: Gestos y animación
description: Los reconocedores son los del sistema, las transformaciones quedan fuera del layout, y la animación corre en el hilo de dibujo de la plataforma — qué se gana con eso y qué hace distinto cada plataforma.
sidebar:
  order: 5
---

Dos cosas de esta página van juntas casi siempre: un gesto dice dónde está un
dedo, y una transformación pone una vista ahí. Las dos son deliberadamente
baratas —ninguna toca el layout— y eso es lo que permite seguir un dedo a sesenta
frames por segundo con el hilo del motor ocupado en otra cosa.

```ts
@Component({
  selector: 'app-card',
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-view
      [backgroundColor]="'#1e2a4a'"
      [borderRadius]="16"
      [translateX]="x()"
      [animate]="settling() ? 220 : null"
      (pan)="onPan($event)"></an-view>
  `
})
export class Card {
  readonly x = signal(0)
  readonly settling = signal(false)

  onPan(event: NativePanEvent) {
    this.settling.set(event.state !== 'move')
    this.x.set(event.state === 'move' ? event.translationX : 0)
  }
}
```

Mientras el dedo está abajo la vista va exactamente donde va el dedo, sin
animación. Cuando el gesto acaba, `animate` se enciende y la plataforma la
devuelve a su sitio. JavaScript no interviene en ninguno de esos sesenta frames.

## Los reconocedores son del sistema

Aquí no se reimplementa ningún gesto. `(longPress)` dispara cuando lo dice
`UILongPressGestureRecognizer`, y el tiempo que hay que esperar es el que esa
versión de iOS considere una pulsación larga. El mismo gesto en Android tiene el
umbral de Android, y en un Mac es mantener el botón del ratón en vez de un dedo.

Eso es una decisión, no un descuido. Una app cuya pulsación larga tarda 400 ms en
una plataforma donde todo lo demás espera 500 ms se siente rara de una forma que
nadie sabe nombrar.

| Output | iOS · iPadOS | macOS |
|---|---|---|
| `(press)` | `UITapGestureRecognizer` | `NSClickGestureRecognizer` — un clic |
| `(doublePress)` | el mismo, dos toques | el mismo, dos clics |
| `(longPress)` | `UILongPressGestureRecognizer` | `NSPressGestureRecognizer` — el botón mantenido |
| `(pan)` | `UIPanGestureRecognizer` | `NSPanGestureRecognizer` — arrastrar con el botón pulsado |
| `(pinch)` | `UIPinchGestureRecognizer` | `NSMagnificationGestureRecognizer` — solo trackpad |
| `(rotation)` | `UIRotationGestureRecognizer` | `NSRotationGestureRecognizer` — solo trackpad |
| `(swipeLeft/Right/Up/Down)` | `UISwipeGestureRecognizer`, uno por dirección | `swipeWithEvent:` — ver abajo |

## Un gesto que nadie escucha no existe

Cada output de gesto es un `outputFromObservable` sobre un observable **frío**: el
reconocedor se engancha cuando Angular se suscribe, y se suelta cuando la vista se
destruye. Angular se suscribe a un output solo si la plantilla lo enlaza, así que
una vista sin `(press)` no cuesta nada — ni reconocedor, ni objeto destino, ni
delegado.

Esto importa a la escala a la que llega una lista. Veinte mil filas con un
`(press)` cada una enganchan veinte mil reconocedores; veinte mil filas dentro de
un contenedor que tiene el `(press)` enganchan uno.

## Los cuatro estados, y por qué `cancel` no es un fallo

`(pan)`, `(pinch)` y `(rotation)` llevan un `state`:

| `state` | Cuándo |
|---|---|
| `begin` | El sistema ha decidido que el gesto cuenta. |
| `move` | Está en curso. La mayoría de los eventos son este. |
| `end` | El dedo se ha levantado y el gesto se ha completado. |
| `cancel` | El sistema se ha llevado el gesto. |

`cancel` llega cuando gana otro reconocedor: arrastrar dentro de una lista que
entonces empieza a hacer scroll es el caso de todos los días. No es un error y no
es `end`: lo que se estuviera moviendo tiene que volver a donde estaba, no
quedarse a medias. En iOS tanto `Cancelled` como `Failed` llegan como `cancel`,
porque desde la plantilla no hay nada que distinguir.

La `translation` se mide **desde donde empezó el dedo**, no desde el evento
anterior. Sumarla a la posición que tenía la vista cuando empezó el gesto es toda
la aritmética que hace falta, sin nada que acumular y sin error de redondeo
arrastrado desde cada paso.

## Las transformaciones no participan en el layout

`translateX`, `translateY`, `scale`, `scaleX`, `scaleY` y `rotate` mueven lo que
se pinta. No mueven lo que se midió: una vista trasladada 200 puntos a la derecha
sigue ocupando el sitio que ocupaba, y sus vecinas no se apartan.

Eso es justo por lo que son las que hay que usar mientras un dedo está abajo: no
hay nada que recalcular, así que el coste es una matriz del lado de la
plataforma. Para mover algo **y** que la vecina se aparte hay que cambiar el
layout: `[style.marginLeft]`, un cambio de orden, un `flexGrow`.

`rotate` va en radianes, que es lo que reporta `(rotation)`, así que los dos se
combinan sin conversión por el medio.

:::note[`[rotate]` manda, `(rotation)` informa]
El output del gesto no podía llamarse también `rotate`: una clase no puede tener
dos miembros con el mismo nombre, y `[rotate]` ya era la transformación. La
diferencia de nombres acabó mereciendo la pena por sí misma.
:::

## `[animate]` es un modo, no una orden

```html
<an-view [animate]="200" [animateEasing]="'ease-in-out'" [translateX]="x()"></an-view>
```

`animate` es un número de milisegundos. Con eso puesto, los **cambios** de esa
vista dejan de ser un salto: moverse, escalar, cambiar de opacidad o ser
recolocada por el layout los interpola la plataforma, en su propio hilo de
dibujo. Se pone una vez y vale para todos los cambios que vengan después. Cero o
`null` lo apaga.

| Prop | Valores | Por defecto |
|---|---|---|
| `animate` | milisegundos | apagado |
| `animateDelay` | milisegundos | `0` |
| `animateEasing` | `linear`, `ease-in`, `ease-out`, `ease-in-out` | `ease-out` |

`ease-out` es el valor por defecto porque es lo que hace casi siempre una
animación del sistema: salir rápido y frenar al llegar.

La razón de que esto sea una prop y no una API es el hilo. Una animación de
`UIView` corre en el render server; un `ValueAnimator` corre en el hilo de UI de
Android. Ninguno de los dos vuelve por JavaScript en ninguno de los frames
intermedios, así que una animación se mantiene suave mientras el hilo del motor
compila una ruta, resuelve un grafo de señales o hace cualquier otra cosa que
tiraría frames si estuviera llevando la animación.

## Lo que no tiene cada plataforma

Un evento que una plataforma no puede entregar **avisa al suscribirse**. No falla
en silencio, y no disimula.

**En la tele no hay pinch ni rotación.** La superficie del Siri Remote es de un
solo toque, y `UIPinchGestureRecognizer` y `UIRotationGestureRecognizer` no están
en el SDK de tvOS. `(pan)` y los cuatro swipes sí funcionan: la superficie
reporta un arrastre. `(press)` llega del botón central, no de un toque — lo que
significa que una vista que no puede coger el foco no se puede pulsar nunca. Eso
es la plataforma, no una limitación de aquí, y está detallado en
[platforms/tvos](/es/platforms/tvos/).

**En el escritorio el swipe no es un reconocedor.** AppKit no trae
`NSSwipeGestureRecognizer`; el gesto existe, pero llega como `swipeWithEvent:`
subiendo por la cadena de respondedores, lo que significa que hay que atenderlo
en la *clase* de la vista y no engancharlo a una vista cualquiera. Solo las
vistas que son de este host pueden cogerlo. Pon un `(swipeLeft)` directamente
sobre un control del sistema —un `NSButton`, un `NSSlider`— y avisa al
suscribirte y te dice dónde ponerlo: en un `an-view` que lo envuelva.

No se imita con un `pan` y un umbral de distancia, por la misma razón por la que
aquí no se imita nada más: el umbral sería nuestro, y el del sistema es el que
usan las demás apps del usuario.

**En un móvil no hay puntero.** `(hover)` y `[cursor]` son solo de escritorio, y
eso está declarado, no olvidado: un dedo no tiene forma y nada sobrevuela antes
de tocar. Donde **sí** hay ratón, los dos se montan con un `NSTrackingArea`, que
a diferencia de un reconocedor no tiene que atenderlo la propia vista, así que
funcionan igual sobre un `NSButton` del sistema que sobre un `an-view`. Si
enganchas `(hover)` en iOS o en Android, el host lo dice al suscribirse la
plantilla, una vez y con el motivo: no se descarta en silencio. Mira
[lo que una plataforma no puede entregar](/es/reference/events/#lo-que-una-plataforma-no-puede-entregar).

## Cuándo se entregan los eventos

Nunca en mitad de un frame. Un evento nativo se encola y se le entrega a
JavaScript al **principio del frame siguiente**, así que todo lo que pasó entre
dos vsyncs se procesa en un turno y produce un commit.

Un arrastre da por tanto un `(pan)` por frame y no uno por muestra táctil, que es
lo que interesa: las muestras de más costarían cada una una pasada de detección
de cambios y producirían un árbol idéntico al que ese frame va a montar de todas
formas.

## El ejemplo

```bash
cargo an dev examples/gestures
```

`examples/gestures` tiene cada uno en su propia vista, imprimiendo la carga según
llega. `scripts/check-gestures.sh` mueve esa misma app a través de `headless` y
comprueba la forma de lo que sale, así que las cargas de esta página son las que
produce el código y no las que se pretendía que produjera.
