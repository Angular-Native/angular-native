---
title: watchOS
description: Angular en el Apple Watch — el único host que no es una jerarquía de vistas, la corona digital, y los ocho primitivos que el SDK no tiene.
sidebar:
  order: 6
---

El mismo core, el mismo layout, el mismo bundle. Lo que cambia es quién pinta.

```bash
an watchos                          # examples/hello-watch en el simulador
an watchos examples/watch-controls  # todo lo que el reloj sabe dibujar
an dev --watchos examples/watch-controls
./scripts/check-watchos.sh
```

## Por qué esto no es el host de iOS con un `cfg`

watchOS no tiene jerarquía de `UIView`. La interfaz es SwiftUI y no hay vuelta
que darle: no hay contenedor al que añadir subvistas ni frame que mover.

Eso choca de frente con el modelo de montaje de este proyecto, que es imperativo
—crear una vista, ponerla aquí, cambiarle el color— mientras que SwiftUI solo
acepta estado: describes lo que hay y él decide qué redibujar.

La salida es reflejar el árbol que mantiene Rust en un modelo que SwiftUI
observa, y convertir cada `MountOp` en una mutación de ese modelo.

```text
  Rust                                   Swift
  ────────────────────────────────       ─────────────────────────────
  QuickJS ─▶ ShadowTree ─▶ taffy         Timer a 30 Hz
                 │ commit                       │ an_watch_runtime_frame
                 ▼                              ▼
           Frame { MountOp[] }            ¿cambió la revisión?
                 │                              │ sí
                 ▼                              ▼
            WatchHost (modelo)  ──JSON──▶  AnTree (@Observable)
                                                 │
                                                 ▼
                                           ZStack + .position
```

`WatchHost` implementa `HostRenderer` pero no posee ninguna vista: un `MountOp`
aterriza en un `HashMap<NodeId, WatchNode>` y cada mutación incrementa un
contador de revisión. Una vez por fotograma, `snapshot()` serializa **el árbol
entero** a JSON — una foto, nunca un diff. Una pantalla de reloj son diez o
quince nodos, así que `Codable` lo descodifica sin parser escrito a mano y un
cruce de FFI sustituye a los cientos que costaría una llamada por `MountOp`.
`AnTree` lee primero la revisión y vuelve enseguida cuando no ha cambiado, así
que un fotograma quieto no descodifica nada y SwiftUI no recompone.

El hilo de JS corre con una pila de 8 MB y un presupuesto de 25 ms por
fotograma: 33 ms a 30 Hz, con 8 ms de sobra para que SwiftUI recomponga.
`CADisplayLink` no existe en watchOS, y por eso el bucle es un `Timer`.

**El layout sigue siendo el de taffy.** No hay ni un `VStack`, ni un `HStack`,
ni un `.padding` en todo el shell. Dos motores de layout decidiendo lo mismo
significa que gana el que corra el último, así que cada nodo se coloca con
`.frame(width:height:)` más `.position` —`.position`, no `.offset`, que
desplazaría la vista desde donde SwiftUI la hubiera centrado— dentro de un
`ZStack(alignment: .topLeading)`, que es el sistema de coordenadas que entrega
taffy. `check-watchos.sh` lo hace cumplir: un `VStack` o un `.padding` que
aparezca en los ficheros que dibujan el árbol suspende la comprobación.

Los colores se resuelven a RGBA 0..1 en Rust. Swift no parsea nunca `#rrggbb`.

## La cadena de herramientas, que es la parte que puede no estar

`aarch64-apple-watchos-sim` es un target de **nivel 3**: rustup lo lista pero no
trae `std` precompilado. Hay que compilarlo en el momento, y eso necesita
nightly:

```bash
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly
```

Por eso `an watchos` llama a `cargo +nightly` en lugar de al toolchain de
`rust-toolchain.toml`, que fija estable con los dos targets de iOS y nada más.
`check-watchos.sh` se salta el paso de compilación cruzada —diciéndolo— cuando
no encuentra un nightly con `rust-src`: que a alguien le falte un toolchain no
debería tumbar el resto de las comprobaciones.

No hay `.xcodeproj`. `an watchos` llama a `xcrun swiftc` directamente con
`-sdk watchsimulator`, `-target arm64-apple-watchos11.0-simulator` y
`-parse-as-library` — sin esa última bandera swiftc trata el `@main` de SwiftUI
como un script de nivel superior y no se usa nunca, en silencio. Las fuentes son
`shells/watchos/Sources` más un fichero de `shells/shared`.

**QuickJS compila y corre en el reloj sin tocar nada.** Era el gran riesgo del
port y no se materializó.

## Los diecisiete que dibuja

Todos son el control del sistema, no un dibujo que se le parece: traen las
hápticas, el resaltado y el comportamiento de la corona que watchOS les da.

| Primitivo | Qué es en el reloj |
|---|---|
| `an-view` | `ZStack(alignment: .topLeading)` — fondo, esquinas (un radio o cuatro), borde, opacidad y los gestos que pida la plantilla. |
| `an-text` | `Text`. Fuente, peso, cursiva, familia, `letterSpacing`, subrayado y tachado, alineación y `numberOfLines`. Medido con la `UIFont` de verdad. |
| `an-button` | `Button` con `.buttonStyle(.plain)`: el resaltado y las hápticas son del sistema, el fondo es de la app. |
| `an-scroll-view` | `ScrollView(.vertical)`. La corona lo desplaza porque SwiftUI lo desplaza — eso no es algo que merezca la pena imitar. |
| `an-image` | Del bundle o de una URL `http(s)`, a través de una caché respaldada por un actor; informa de su tamaño natural con `(load)`, que es lo que necesita el layout para colocarla. |
| `an-icon` | `Image(systemName:)`. El nombre se traduce a un SF Symbol en Rust, en `an_core::icons`. |
| `an-switch` | `Toggle().labelsHidden()`. |
| `an-slider` | `Slider`. En el reloj viene con el menos y el más a los lados, que es su forma allí. |
| `an-stepper` | `Stepper`, los dos botones del sistema. |
| `an-progress-bar` | `ProgressView(value:)`. |
| `an-activity-indicator` | `ProgressView()` indeterminado; con `[animating]` a falso pasa a ser `Color.clear`, como se comporta `hidesWhenStopped`. |
| `an-text-input` | `TextField` / `SecureField`. Tocarlo abre la pantalla de entrada **propia** del reloj —dictado, escritura a mano o teclado— y devuelve el texto. |
| `an-select` | `Picker().labelsHidden()`: la rueda que gira la corona. En un reloj no hay desplegable. |
| `an-date-picker` | `DatePicker`, el selector de esfera del reloj. |
| `an-stack-view` | `ZStack` mostrando el último hijo, con transiciones: `pop` entra por delante y sale por detrás, `none` es la identidad, y por defecto es la inversa. `.easeOut` en 0,25 s. |
| `an-alert` | `.alert`, con sus botones y su `(select)`. Sin botones se le pone un `OK`. |
| `an-modal` | `.sheet`, o `.fullScreenCover` cuando `[presentation]` es `fullScreen`. |

Dos cosas que una plantilla tiene que saber:

- **`an-text-input` necesita `[style.height]`.** taffy lo mide como mide un
  `<Text>` —una línea— porque en iOS un `UITextField` sin borde es exactamente
  eso. En el reloj el campo siempre se dibuja dentro de su propio contenedor
  redondeado, de **40 puntos** de alto sea cual sea el frame que se le dé, así
  que sin una altura explícita se come la fila de abajo. El shell lo registra
  una vez cuando el frame llega más bajo que eso. `.textFieldStyle(.plain)` no
  quita el contenedor: se probó en watchOS 26 y no cambia nada.
- **`an-alert` y `an-modal` salen del árbol.** En SwiftUI no son vistas que
  coloques, son modificadores de la raíz, así que Rust los envía aparte, en
  `overlays`, y el shell los cuelga de `RootView`. Un `an-modal` **sí** sigue
  ocupando sitio en el layout —eso lo decide el core, exactamente igual que en
  iOS— y en una pantalla de 248 puntos de alto eso es media pantalla: dale
  `[style.position]="'absolute'"` con el tamaño de la pantalla, que es también
  el frame en el que se maqueta su contenido.

## Los ocho que no

Con el motivo, que casi siempre es del SDK y no una opinión. Cuando el árbol
pide uno de estos, el nodo no se crea, se deja el hueco que midió el layout, y
el host lo dice una vez — con el motivo, no con una caja etiquetada. El shell
dibuja `Color.clear` al tamaño maquetado a propósito.

| Primitivo | Por qué no |
|---|---|
| `an-tab-bar` | Una barra de pestañas no cabe en 205 puntos de ancho. Lo que hace un reloj es deslizar entre secciones a pantalla completa — un contenedor, no una barra con un frame, así que es otro primitivo. |
| `an-navigation-bar` | La franja de arriba de un reloj ya es del sistema: la hora y el título de la app. Una barra nuestra pintaría debajo o encima. |
| `an-segmented-control` | `SegmentedPickerStyle` es `@available(watchOS, unavailable)` en SwiftUI. Lo que usa el reloj en su lugar es `an-select`. |
| `an-search-bar` | La búsqueda en un reloj es una pantalla del sistema, no un campo con una lupa. `.searchable` existe, pero es un modificador de navegación, no una vista con marco. |
| `an-textarea` | `TextEditor` es `@available(watchOS, unavailable)`. El texto largo se dicta o se escribe a mano, y `an-text-input` ya te da eso. |
| `an-web-view` | WebKit no está en el SDK de watchOS. |
| `an-map-view` | El `Map` de SwiftUI sí existe en watchOS, pero no acepta ni centro ni zoom de la app: mostraría un lugar que la plantilla no eligió. |
| `an-video-view` | AVKit en watchOS no trae ni `AVPlayerViewController` ni `VideoPlayer`. Sus cabeceras declaran tipos y ninguna vista de reproducción. |

`check-watchos.sh` comprueba que la lista de soportados y la de no soportados
cubren juntas todo el vocabulario, y que ningún primitivo aparece en las dos ni
en ninguna.

## La corona digital

Es el control propio del reloj y el único sin equivalente en un teléfono:
analógico, con inercia y con hápticas, y usado sin que un dedo tape la pantalla.
Una plantilla la pide como cualquier otro evento:

```html
<an-view (crown)="turned($event)" (crownIdle)="stopped()"> … </an-view>
```

| Clave | Qué es |
|---|---|
| `delta` | Cuánto ha girado desde el evento anterior. |
| `offset` | Acumulado desde que la vista tomó la corona. |
| `velocity` | Con signo, en vueltas por segundo — la `velocity` de SwiftUI. |

`delta` no es algo que dé SwiftUI: `DigitalCrownEvent` lleva `offset` y
`velocity`, y el shell resta. Lo que casi siempre quiere una plantilla es «mueve
el valor lo que haya girado», y hacer esa resta en cada plantilla significaría
repetirla en cada plantilla. `(crownIdle)` llega cuando se para, y es
deliberadamente otra cosa que un `(crown)` con delta cero — el `onIdle` de
SwiftUI es un callback aparte y no hay nada que inventar.

El modificador se engancha solo a nodos contenedores, y solo cuando el nodo
escucha `crown` de verdad.

Tres cosas que conviene saber, y las tres costaron averiguarlas:

- **La corona va a quien tiene el foco, y el foco es uno.** Eso no es una
  decisión del shell, es cómo funciona watchOS. Una vista con `(crown)` se
  declara `focusable` y pide el foco inicial con `defaultFocus`, así que si
  nadie más lo tiene, se lo queda. Con varias vistas `(crown)`, gana la primera
  en orden de pintado.
- **Asignar el `FocusState` a mano no funciona.** Se probó desde la raíz y desde
  el `onAppear` del propio nodo: SwiftUI se traga la asignación sin decir nada
  si la vista todavía no está en pantalla, y la corona se queda muerta sin que
  nada lo diga. Lo que funciona es `defaultFocus`.
- **Un `ScrollView` se queda la corona.** Si la vista con `(crown)` está dentro
  de uno, la corona desplaza la lista hasta que se toca la vista. Todas las apps
  de reloj se comportan así, pero conviene saberlo:
  `examples/watch-controls` pone su pantalla de corona **fuera** de un
  `an-scroll-view` a propósito. Tocar un `Slider`, un `Stepper` o un `Picker`
  también mueve el foco a ese control, y hay que volver a tocar la vista para
  recuperar la corona.

Los controles que la usan —`an-slider`, `an-stepper`, `an-select`— la reciben
del sistema sin pedir nada: cuando tienen el foco, la corona los mueve. El
acumulador de la corona se guarda en un diccionario separado del estado de los
controles, porque un mismo nodo puede ser a la vez un `an-slider` y un oyente de
`(crown)`.

## Gestos

| Gesto | En el reloj |
|---|---|
| `(press)` | `onTapGesture`. En un `an-button` lo envía el propio `Button`, con su resaltado y sus hápticas. |
| `(doublePress)` | `onTapGesture(count: 2)`. |
| `(longPress)` | Un `LongPressGesture` de medio segundo, en un `simultaneousGesture`. |
| `(pan)` | `DragGesture`, con `translation` y `velocity`. Las fases son `begin`, `move` y `end`. |
| `(swipeLeft)`, `(swipeRight)`, `(swipeUp)`, `(swipeDown)` | El mismo `DragGesture`, leyendo el balance al soltar: 24 puntos o más en el eje dominante. |
| `(crown)`, `(crownIdle)` | La corona. |

Solo se engancha lo que pide la plantilla. Un recogniser de más se come el
arrastre del `ScrollView` de debajo, y watchOS resalta lo que cree que se puede
tocar, así que envolverlo todo en un gesto haría parpadear media pantalla al
rozarla.

**`(longPress)` y el arrastre no pueden ser dos gestos separados.** Con
`.onLongPressGesture` sobre una vista que además escuchaba `(swipeLeft)`, la
pulsación larga no llegaba nunca: los dos pelean por el dedo y ganaba el
arrastre. Va en un `simultaneousGesture`, y la posición del toque sale del
propio arrastre y no de un segundo `DragGesture` de distancia cero, que era el
mismo problema otra vez.

Y lo que no llega, avisado una vez por gesto:

| Gesto | Por qué no |
|---|---|
| `(pinch)`, `(rotation)` | `MagnifyGesture` y `RotateGesture` son `@available(watchOS, unavailable)`. Dos dedos no caben en 40 mm. |
| `(back)` | Fuera de un `NavigationStack` el reloj no da arrastre de borde, y montar uno metería el layout de SwiftUI dentro del de taffy. |
| `(refresh)` | En un reloj no se tira de una lista para recargar: eso es la corona, que ya llega como `(crown)`. |
| `(scroll)` | El `ScrollView` de SwiftUI no publica su desplazamiento en watchOS 11, que es el mínimo de este shell. |
| `(safeArea)` | La app es dueña de toda la pantalla y el sistema no reserva ningún margen consultable — así que `an-safe-area` se monta y no hace nada ahí. |
| `(focus)`, `(blur)` | Todavía no: el foco del reloj es el mismo foco que decide quién tiene la corona, y dos dueños la harían saltar. |

## El estado que mueve un dedo

Esta es la parte que no se ve y la más fácil de romper. Los dos lados llevan el
mismo valor a distintos ritmos: un `Slider` de SwiftUI necesita un `Binding` en
el que pueda escribir de inmediato —el dedo está encima— mientras el valor de
verdad vive en una señal de Angular al otro lado de QuickJS y no vuelve hasta el
siguiente fotograma. Sin algo en medio, el slider saltaría hacia atrás en cada
arrastre, porque cada foto lo devolvería al valor viejo.

Ese algo es `AnControls`, y tiene una regla:

> si el valor que llega de Rust **es distinto del que llegó la vez anterior**,
> lo cambió la app, y gana la app. Si es el mismo, gana lo que haya hecho el
> dedo.

Así una señal que cambia el valor desde código aparece al momento, y un arrastre
no lo sobreescribe el eco de su propio evento.

Las presentaciones necesitan lo mismo: pulsar un botón de un `an-alert` lo
cierra en SwiftUI mientras `[visible]` sigue siendo `true` hasta que JS reacciona
al `(select)`, y el fotograma de en medio lo reabriría. El diálogo se anota como
cerrado hasta que la plantilla se pone al día.

## Recarga en caliente

`an dev --watchos` funciona igual que en el teléfono: la app conecta al servidor
por WebSocket y guardar recarga el bundle sin reiniciar. El simulador de reloj
comparte la red del Mac, así que el bundle se sirve en `127.0.0.1:<puerto>`. El
comando no tiene un `--device` propio — un `--device` que se haya quedado en el
teléfono por defecto se cambia por el reloj por defecto.

**El estado sobrevive**, y esa fue la pelea que valía la pena ganar, porque el
árbol se reconstruye entero treinta veces por segundo. Sobrevive porque en una
recarga en caliente el core deja el árbol montado bajo los mismos ids, así que
`AnControls` no se entera; y cuando la recarga es en frío el host limpia el
árbol, los ids desaparecen, y la reconciliación se lleva por delante lo que ya
no existe. No hay que vaciar nada a mano, que es exactamente donde esto se
habría roto.

## Manejar el reloj desde fuera

`xcrun simctl` **no tiene ningún verbo para esto**: hay `io … screenshot`,
`io … recordVideo`, `ui`, `spawn`, `push`… y ninguno envía un toque ni un giro.
Es la misma situación que el mando del Apple TV, y la salida es la misma que la
de `tv-remote.sh`: mover el propio ratón del Simulator.

```bash
./scripts/watch-input.sh tap 104 200          # un toque, en puntos de reloj
./scripts/watch-input.sh hold 104 120 900     # una pulsación larga
./scripts/watch-input.sh drag 104 220 104 60  # un arrastre: desplaza una lista
./scripts/watch-input.sh turn -12             # doce pasos de corona
./scripts/watch-input.sh shot /tmp/a.png
```

Lo que más costó encontrar, y está escrito arriba del script: **la rueda del
ratón solo gira la corona mientras el puntero está sobre el botón «Crown» de la
ventana**, no sobre la pantalla. Sobre la pantalla no pasa absolutamente nada
—ni un aviso— y es fácil concluir que la corona no se puede manejar desde fuera.

Tiene las mismas dos condiciones que el mando de la tele: la pantalla del Mac no
puede estar bloqueada, y el Simulator se queda en primer plano mientras corre el
script.

## Nada falla en silencio

Cuatro avisos, cada uno una vez por caso y nunca una vez por fotograma — a
30 Hz, un aviso por fotograma es un log ilegible:

- **Una prop que nadie lee.** La lista blanca por tipo está en `reads()`, junto
  al código que la usa; `[cursor]`, para el que solo el host de escritorio tiene
  un puntero, es una de las props que aterrizan aquí hoy. Los prefijos `ios:` y
  `android:` y `ng-version` están exentos.
- **Una prop que no se puede pintar**, con el motivo, en `unpaintable()`. A
  propósito no es el mismo mensaje que el anterior: «esto todavía no lo lee
  nadie» es un hueco que alguien puede cerrar y «aquí no hay nada sobre lo que
  dibujar» no lo es, y quien no pueda distinguirlos se queda esperando una
  versión que no va a llegar. El caso que existe hoy es un `[borderWidth]` sobre
  un `an-alert`: el diálogo lo presenta el sistema y la app le entrega un
  título, un mensaje y unos botones, no un marco.
- **Un evento que no se puede entregar**, avisado una vez por tipo y nombre de
  evento.
- **Un primitivo que no se dibuja**, con el motivo del SDK.

Las cargas de los eventos son planas y nada más. Un objeto anidado o un array se
rechaza con un log, porque el tipo de valor del cable no los puede representar.

## Módulos nativos

`device` está registrado, así que `Device.info()` resuelve y `'watchos'` —un
valor de `NativePlatform` que hasta entonces no producía ningún host— es un
valor que una app puede leer de verdad.

Los cuatro campos que responde vienen **del shell**, entregados a
`an_watch_runtime_new` como JSON junto a los tamaños de los controles:
`systemVersion`, `model` y `scale` son de `WKInterfaceDevice`, que es WatchKit,
no tiene binding de Rust, y serían cuatro viajes por Objective-C para lo que
Swift resuelve en una línea. Android hace exactamente esto con
`AnHost.deviceInfo()`.

`platform` es el único campo que el shell **no** envía. Esa es la palabra del
crate —este host no puede estar corriendo en otro sitio que un reloj— por el
mismo motivo por el que `an-ios` lo toma del `cfg`: preguntárselo a otro solo
sería sitio para equivocarse. Si no se entrega nada, la llamada se rechaza
diciéndolo, en lugar de responder un objeto con agujeros que se leería como
datos reales.

## Lo que falta

- **Plugins.** `an-watch` no tiene registro, así que `an watchos` se niega a
  compilar una app que dependa de uno en lugar de entregar una app en la que
  todas las llamadas se rechazarían en tiempo de ejecución. `device` no es uno:
  está compilado dentro del host y funciona aquí (mira arriba). Un nombre de
  módulo alcanzado en tiempo de ejecución se rechaza con el nombre *y* el motivo
  de que no haya nada debajo, que no es el mensaje que recibiría una errata.
- **No se pueden meter recursos en el `.app`.** `an watchos` copia el
  `Info.plist` y `main.js` y nada más, así que una `an-image` con un `[source]`
  sin esquema no encuentra el fichero y lo dice en el log. Una URL `http` sí
  funciona —comprobado en el simulador contra un PNG servido desde el Mac— y por
  eso `examples/watch-controls` no lleva ninguna imagen: un ejemplo de este
  repositorio no puede depender de una URL. El sitio donde arreglarlo es
  `watchos.rs::assemble`, y es el único host al que le queda el hueco: iOS,
  macOS y Android ya copian el directorio `resources/` de la app al bundle.
- **Animación y transformaciones.** `[animate]`, `translateX`, `scale`,
  `rotate`. En SwiftUI esto es `withAnimation` y `.offset`/`.scaleEffect`, pero
  el modelo se reconstruye entero en cada foto y una animación necesita saber de
  dónde venía. La stack view ya anima la entrada y la salida de sus pantallas,
  que es el caso más visible.
- **Bordes por lado.** No los hay, ni aquí ni en ninguna otra parte del
  proyecto. `[borderWidth]` es un número y se dibuja; `borderTopWidth` y sus
  tres hermanos son estilos *de layout* — los resuelve taffy, meten hacia dentro
  a los hijos y no llegan a ningún host como algo que pintar. SwiftUI tampoco
  tiene una forma para ellos, así que dibujarlos solo en el reloj sería juntar
  cuatro rectángulos y hacer que el reloj enseñe un borde que el teléfono no
  enseña.
- **La foto entera en lugar de las mutaciones.** Cada fotograma cambiado
  serializa el árbol entero. Con diez o quince nodos no se nota. Cuando aparezca
  una lista larga habrá que mandar las ops por su cuenta, y lo que cambia es
  `snapshot.rs` y el `Decodable` de Swift, no el host.
- **Complicaciones y notificaciones.** Otra superficie del sistema, con su
  propio ciclo de vida. No comparten nada con esto.
- **Un dispositivo real.** No hay camino de dispositivo por ninguna parte: ni
  target `aarch64-apple-watchos`, ni firma, ni `devicectl`. Todo lo de aquí se
  vio en el simulador, en un Apple Watch Series 11 (46 mm), el dispositivo por
  defecto.

## Accesibilidad, y lo único que no se puede demostrar

Los roles, los rasgos y los valores se traducen en Rust y en Swift solo se
enganchan, en cada nodo en lugar de por `case`. Qué se aplica y qué se rechaza
está en [Accesibilidad en Apple](/es/accessibility/apple/).

El hueco que merece decirse aquí es de pruebas: los otros tres hosts de Apple se
pueden leer desde fuera recorriendo el árbol de accesibilidad. watchOS no —no
hay ninguna ruta hacia el árbol de un simulador de reloj— así que nada demuestra
que un lector de pantalla anunciaría nada de esto. El test de Rust solo
comprueba que la foto lleva los nombres correctos.

## Muros que se esperaban y no estaban

Documentados porque estaban en la lista de cosas que podían frenar esto:

- **El simulador acepta el bundle sin firmar.** `simctl install` no pide ninguna
  firma, igual que iOS. Un dispositivo real sí la pediría.
- **`@main` sobre una `App` de SwiftUI funciona con `swiftc` pelado**, sin
  `.xcodeproj`, siempre que se pase `-parse-as-library`.
- **El `Info.plist` necesita `WKApplication`.** Es lo que distingue una app de
  watchOS 7 en adelante del viejo par «WatchKit App + WatchKit Extension». Sin
  esa clave `simctl` instala el bundle y luego no encuentra nada que lanzar. El
  plist lleva además `WKWatchOnly` —independiente, sin app de iPhone
  acompañante—, `UIDeviceFamily [4]` y solo vertical.
- **La medición del texto es la de verdad.** watchOS trae un UIKit recortado sin
  `UIView` pero **con** `UIFont` y el dibujado de cadenas de Foundation, así que
  el medidor del reloj mide lo mismo que el de iOS y con la misma tabla de
  pesos: CSS 100..900 mapeado sobre el -0,8..0,62 de UIKit. Fuera del reloj cae
  al medidor ingenuo para que `cargo test` corra en un Mac. Los tamaños de los
  controles no se pueden preguntar —no hay ningún `UISwitch` al que consultar—
  así que Swift los fija en el código y los pasa al arrancar.
