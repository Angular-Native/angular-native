# watchOS

Angular corriendo en el reloj: mismo núcleo, mismo layout, mismo bundle. Lo que
cambia es quién pinta.

```bash
cargo an watchos                          # examples/hello-watch en el simulador
cargo an watchos examples/watch-controls  # todo lo que el reloj sabe pintar
cargo an dev --watchos examples/watch-controls   # con refresco en caliente
./scripts/check-watchos.sh
```

## Por qué no es el host de iOS con otro `cfg`

watchOS no tiene jerarquía de `UIView`. La interfaz es SwiftUI y no hay forma de
saltárselo: no existe un contenedor al que meterle subvistas y moverles el
marco.

Eso choca de frente con el modelo de montaje del proyecto, que es imperativo
—crea una vista, métela aquí, cámbiale el color— mientras que SwiftUI solo
admite estado: se le describe qué hay y él decide qué redibujar.

La salida es reflejar el árbol que mantiene Rust en un modelo que SwiftUI
observe, y convertir cada `MountOp` en una mutación de ese modelo:

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

**El layout sigue siendo de taffy.** Swift coloca cada nodo por el marco que ya
viene calculado y no usa `VStack`/`HStack`/`padding` para posicionar nada. Si
los usara habría dos motores de layout decidiendo lo mismo y ganaría el que
corriera después. Por eso todo el shell se apoya en `.frame` + `.position`
dentro de un `ZStack` con `alignment: .topLeading`, que es el sistema de
coordenadas que da taffy. `check-watchos.sh` lo vigila: si aparece un `VStack`
o un `.padding` en los ficheros que pintan el árbol, falla.

## El toolchain, que es la parte que puede no estar

`aarch64-apple-watchos-sim` es un target de **nivel 3**: rustup lo lista pero no
trae `std` precompilada. Hay que construirla en el momento, y eso pide nightly:

```bash
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly

cargo +nightly build -Z build-std=std,panic_abort \
  -p an-watch --target aarch64-apple-watchos-sim
```

Es la razón por la que `an watchos` llama a `cargo +nightly` en vez de al
toolchain del `rust-toolchain.toml`, y por la que `check-watchos.sh` se salta la
compilación cruzada —avisando— si no encuentra nightly con `rust-src`: que a
alguien le falte un toolchain no tiene por qué tumbar las demás comprobaciones.

`rquickjs-sys` no envía bindings pregenerados para watchOS, igual que no los
envía para iOS ni Android. Se generan con bindgen, que es lo que ya hacía el
resto: watchOS solo tuvo que entrar en la misma lista de `an-bridge/Cargo.toml`.

**QuickJS compila y corre en el reloj sin tocar nada.** Era el riesgo grande del
port y no se materializó.

## Qué pinta el reloj

Las veinticinco primitivas del núcleo, decididas una por una. La lista de las
que no se pintan está en
[`an-watch/src/snapshot.rs`](../crates/an-watch/src/snapshot.rs) —`unsupported()`—
y la de las que sí, en el `switch` de
[`AnNodeView.swift`](../shells/watchos/Sources/AnNodeView.swift);
`check-watchos.sh` comprueba que entre las dos está el vocabulario entero y que
ninguna primitiva sale en las dos ni en ninguna.

### Las diecisiete que sí

Todas son el control del sistema, no un dibujo que se le parezca: traen el
háptico, el realce y el giro de la corona que trae watchOS.

| Primitiva | Qué es en el reloj |
|---|---|
| `an-view` | Contenedor: fondo, esquinas, opacidad y los gestos que la plantilla pida. |
| `an-text` | Fuente, peso, cursiva, familia, `letterSpacing`, subrayado y tachado, alineación y `numberOfLines`. Medido con el `UIFont` de verdad. |
| `an-button` | `Button` de SwiftUI con `buttonStyle(.plain)`: el realce y el háptico son del sistema, el fondo lo pone la app. |
| `an-scroll-view` | `ScrollView`. La corona lo desplaza porque lo desplaza SwiftUI. |
| `an-image` | `Image`, del bundle o de una URL; devuelve su tamaño natural por `(load)`, que es lo que el layout necesita para colocarla. |
| `an-icon` | SF Symbol pedido por nombre, con el trazo que le toque a esa versión de watchOS. |
| `an-switch` | `Toggle` con estilo de interruptor. |
| `an-slider` | `Slider`. En el reloj sale con el menos y el más a los lados, que es su forma allí. |
| `an-stepper` | `Stepper`, los dos botones del sistema. |
| `an-progress-bar` | `ProgressView(value:)`. |
| `an-activity-indicator` | `ProgressView()` indeterminado; con `[animating]` a `false` desaparece, como `hidesWhenStopped`. |
| `an-text-input` | `TextField` / `SecureField`. Al tocarlo, el reloj abre **su** pantalla de entrada —dictado, garabateo o teclado— y devuelve el texto. |
| `an-select` | `Picker`: la rueda que gira con la corona. En el reloj no hay desplegable. |
| `an-date-picker` | `DatePicker`, el selector de esferas de watchOS. |
| `an-alert` | `.alert` del sistema, con sus botones y su `(select)`. |
| `an-modal` | `.sheet` o `.fullScreenCover` según `[presentation]`. |
| `an-stack-view` | Pila de pantallas: solo se ve la de arriba, y entra o sale según `[transition]`. |

Dos avisos que la plantilla tiene que conocer:

- **`an-text-input` necesita `[style.height]`.** taffy lo mide como mide un
  `<Text>` —una línea— porque en iOS un `UITextField` sin borde ocupa
  exactamente eso. En el reloj el campo se dibuja siempre dentro de su propio
  contenedor redondeado, más alto, y sin altura explícita se come la fila de
  debajo. Cuarenta y cuatro puntos es lo que usa watchOS. `.textFieldStyle(.plain)`
  no lo quita: se probó, y en watchOS 26 no cambia nada. El host lo dice por el
  registro si el marco viene más bajo.
- **`an-alert` y `an-modal` salen del árbol.** En SwiftUI no son vistas que se
  coloquen, son modificadores sobre la raíz, así que Rust los manda aparte, en
  `overlays`. Un `an-modal`, en cambio, **sí ocupa sitio en el layout** —eso lo
  decide el núcleo, igual que en iOS—, y en 248 puntos de alto eso es media
  pantalla: hay que ponerlo `[style.position]="'absolute'"` con el tamaño de la
  pantalla, que es además el marco con el que se coloca lo de dentro.

### Las ocho que no

Con el motivo, que casi siempre es del SDK y no una opinión. Cuando el árbol
pide una de estas, queda el hueco que dijo el layout y el host lo dice una vez,
con este texto:

```
angular-native: TabBar no se pinta en watchOS: una barra de pestañas no cabe en
205 puntos: en el reloj las secciones se pasan con el dedo a pantalla completa…
```

| Primitiva | Por qué no |
|---|---|
| `an-tab-bar` | Una barra de pestañas no cabe en 205 puntos de ancho. Lo que hace el reloj es pasar de sección con el dedo, a pantalla completa (`TabView` con `.verticalPage`), que es un contenedor y no una barra con marco: es otra primitiva, no esta. |
| `an-navigation-bar` | La franja de arriba del reloj ya es del sistema —la hora y el título de la app—. Una barra propia se pintaría debajo o encima de ella. |
| `an-segmented-control` | `SegmentedPickerStyle` está marcado `@available(watchOS, unavailable)` en SwiftUI. Lo que el reloj usa para lo mismo es `an-select`. |
| `an-search-bar` | En el reloj buscar es una pantalla del sistema, no un campo con lupa. `.searchable` existe, pero es un modificador de navegación, no una vista con marco. |
| `an-textarea` | `TextEditor` está marcado `@available(watchOS, unavailable)`. El texto largo se dicta o se garabatea, y eso ya lo da `an-text-input`. |
| `an-web-view` | WebKit no está en el SDK de watchOS. |
| `an-map-view` | `Map` sí existe (watchOS 7+), pero en el reloj no acepta ni centro ni zoom desde la app: enseñaría un sitio que la plantilla no eligió, que es peor que no enseñar nada. |
| `an-video-view` | AVKit en watchOS no trae `AVPlayerViewController` ni `VideoPlayer`: sus cabeceras solo declaran tipos, ninguna vista de reproducción. |

## La corona digital

Es el mando propio del reloj y el único que no tiene equivalente en el teléfono:
analógico, con inercia y con háptico, y se usa sin tapar la pantalla con el
dedo. Hasta ahora solo movía el `ScrollView`, que es lo que hace SwiftUI sola.
Ahora una plantilla puede pedirla:

```html
<an-view (crown)="gira($any($event))"> … </an-view>
```

El evento lleva:

| clave | qué es |
|---|---|
| `delta` | cuánto ha girado desde el aviso anterior |
| `offset` | el acumulado desde que la vista tomó la corona |
| `velocity` | con signo; es el `velocity` de `DigitalCrownEvent` |

`delta` no lo da SwiftUI —`DigitalCrownEvent` trae `offset` y `velocity`— y se
calcula restando en el shell, porque lo que casi siempre se quiere es «súbeme el
valor lo que ha girado», y hacer esa resta en cada plantilla sería repetirla en
cada plantilla. Cuando se para llega un `(crownIdle)`, que es una cosa distinta
de un `(crown)` con delta cero.

Tres cosas que hay que saber y que costó descubrir:

- **La corona va a quien tiene el foco, y el foco es uno.** No es una decisión
  del shell: es cómo funciona watchOS. Una vista con `(crown)` se declara
  `focusable` y pide el foco de salida con `defaultFocus`, así que si no lo
  tiene nadie, se lo queda ella.
- **Asignar el `FocusState` a mano no vale.** Se probó desde la raíz y desde el
  `onAppear` del propio nodo: SwiftUI se traga la asignación sin avisar si la
  vista todavía no está en pantalla, y la corona queda muerta sin que nada lo
  diga. `defaultFocus` es lo que funciona.
- **Un `ScrollView` se queda la corona.** Si la vista con `(crown)` está dentro
  de uno, la corona desplaza la lista hasta que se toca la vista. Es lo que
  hace cualquier app del reloj, pero conviene saberlo: `examples/watch-controls`
  pone su pantalla de corona **fuera** de un `an-scroll-view` a propósito.

Los controles que la usan —`an-slider`, `an-stepper`, `an-select`— la reciben
del sistema sin pedir nada: cuando tienen el foco, la corona los mueve.

## Gestos

| Gesto | En el reloj |
|---|---|
| `(press)` | `onTapGesture`. Sobre un `an-button` lo manda el propio `Button`, con su realce y su háptico. |
| `(doublePress)` | `onTapGesture(count: 2)`. |
| `(longPress)` | `LongPressGesture` de medio segundo, en `simultaneousGesture`. |
| `(pan)` | `DragGesture`, con `translation` y `velocity`. Las fases son `begin`, `move` y `end`. |
| `(swipeLeft)`, `(swipeRight)`, `(swipeUp)`, `(swipeDown)` | El mismo `DragGesture`, mirando el saldo al soltar: 24 puntos o más en el eje dominante. |
| `(crown)`, `(crownIdle)` | La corona. |

Solo se engancha lo que la plantilla pide. Un reconocedor de más se come el
arrastre del `ScrollView` de debajo, y watchOS realza lo que cree tocable, así
que envolver todo en un gesto haría parpadear media pantalla al rozarla.

**El `(longPress)` y el arrastre no pueden ser dos gestos sueltos.** Con
`.onLongPressGesture` sobre una vista que además escuchaba `(swipeLeft)`, la
pulsación larga no llegaba nunca: los dos se disputan el dedo y ganaba el
arrastre. Va en `simultaneousGesture`, y la posición del toque sale del propio
arrastre en vez de un segundo `DragGesture` de distancia cero, que era otra vez
el mismo problema.

Y lo que no llega, que también se dice una vez por gesto:

| Gesto | Por qué no |
|---|---|
| `(pinch)`, `(rotation)` | `MagnifyGesture` y `RotateGesture` están marcados `@available(watchOS, unavailable)`. En 40 mm no caben dos dedos. |
| `(back)` | Fuera de un `NavigationStack` el reloj no da el arrastre desde el borde, y montar uno metería el layout de SwiftUI dentro del de taffy. |
| `(refresh)` | En el reloj no se tira de una lista para recargar: eso se hace con la corona, que ya llega como `(crown)`. |
| `(scroll)` | El `ScrollView` de SwiftUI no publica su desplazamiento en watchOS 11, que es el mínimo de este shell. |
| `(safeArea)` | La app ocupa la pantalla entera y el sistema no reserva márgenes que se puedan preguntar. |
| `(focus)`, `(blur)` | Todavía no: el foco del reloj es el mismo que decide quién tiene la corona, y darle dos dueños haría que la corona saltase de sitio al escribir. |

## El estado que el dedo mueve

Es la parte que no se ve y la que más fácil se rompe. Los dos lados llevan el
mismo dato y no van al mismo ritmo: un `Slider` de SwiftUI necesita un `Binding`
que pueda escribir en el acto —el dedo está encima— mientras que el valor de
verdad vive en una señal de Angular, al otro lado de QuickJS, y no vuelve hasta
el frame siguiente. Sin un intermedio, el deslizador saltaría hacia atrás en
cada arrastre porque cada foto lo devolvería al valor viejo.

Ese intermedio es [`AnControls`](../shells/watchos/Sources/AnControls.swift), y
su regla es una sola:

> si el valor que llega de Rust **es distinto del que llegó la última vez**, lo
> cambió la app, y manda la app. Si es el mismo, manda lo que haya tocado el
> dedo.

Así una señal que cambia el valor desde código se ve enseguida, y un arrastre no
se pisa con el eco de su propio evento.

Lo mismo hace falta para las presentaciones: al pulsar un botón de un
`an-alert`, SwiftUI lo cierra pero `[visible]` sigue valiendo `true` hasta que
JS reacciona al `(select)`, y el frame de en medio lo volvería a abrir. El
diálogo se apunta como cerrado hasta que la plantilla se entera.

## El refresco en caliente

`an dev --watchos` funciona igual que en el teléfono: la app se conecta por
WebSocket al servidor, y al guardar un cambio el bundle se recarga sin
reiniciar.

**El estado sobrevive**, y esa era la pelea que había que ganar: el árbol se
rehace entero en cada foto, treinta veces por segundo. Sobrevive porque en una
recarga en caliente el núcleo mantiene el árbol montado con los mismos ids, así
que `AnControls` no se entera de nada; y cuando la recarga es en frío el host
vacía el árbol, los ids desaparecen y la reconciliación se lleva por delante lo
que ya no existe. No hay que acordarse de vaciar nada a mano, que es justo el
sitio donde esto se habría roto.

Visto: con el contador de la corona en 14, cambiar el rótulo del componente y
guardar deja el rótulo nuevo y el 14 donde estaba, en la misma pantalla de la
pila.

## Mover el reloj desde fuera

`xcrun simctl` **no tiene ningún verbo para esto**: están `io … screenshot`,
`io … recordVideo`, `ui`, `spawn`, `push`… y ninguno manda un toque ni un giro.
Es lo mismo que pasaba con el mando del Apple TV, y la salida es la misma que en
`tv-remote.sh`: mover el ratón del propio Simulator.

```bash
./scripts/watch-input.sh tap 104 200          # toque, en puntos del reloj
./scripts/watch-input.sh hold 104 120 900     # pulsación larga
./scripts/watch-input.sh drag 104 220 104 60  # arrastre: desplaza una lista
./scripts/watch-input.sh turn -12             # doce pasos de corona
./scripts/watch-input.sh shot /tmp/a.png
```

Lo que más costó encontrar, y por eso está escrito en la cabecera del script:
**la rueda del ratón solo gira la corona si el puntero está encima del botón
«Crown» de la ventana**, no encima de la pantalla. Sobre la pantalla no pasa
absolutamente nada —ni un aviso— y es fácil concluir que la corona no se puede
mover desde fuera.

Tiene las mismas dos condiciones que el mando de la tele: la pantalla del Mac no
puede estar bloqueada, y Simulator se queda en primer plano mientras el script
corre.

## Nada falla en silencio

Tres avisos, todos una vez por caso y nunca por frame —a 30 Hz un aviso por
frame es un registro ilegible—:

- **Una prop que nadie mira.** `<an-button> recibió [variant], y el host de
  watchOS no la mira`. La lista de lo que sí se lee está en `reads()`, junto al
  código que la usa.
- **Un gesto que no llega.** `(pinch) no llega en watchOS: MagnifyGesture está
  marcado @available(watchOS, unavailable)…`.
- **Una primitiva que no se pinta**, con el motivo del SDK.

## Qué falta

- **`(crown)` no lo declara ninguna directiva.** Funciona —Angular pasa
  cualquier `(evento)` al renderer— pero el `$event` sale tipado como `Event`, y
  la plantilla necesita un `$any`. Arreglarlo es una línea en `NativeVisual`
  (`packages/primitives`): una salida `crown` con su interfaz de carga. Es la
  misma deuda que tiene tvOS con `(focus)` y `(blur)`.
- **La tabla de iconos vive en dos sitios.** La copia buena está ahora en
  `an-core::icons` y la usa el reloj; `an-ios` sigue con la suya, privada.
  Mientras estén las dos, `check-watchos.sh` comprueba que dicen lo mismo, pero
  lo que corresponde es que `an-ios` use la del núcleo y se borre la suya.
- **No hay forma de meter un recurso en el `.app`.** `an watchos` copia el
  `Info.plist` y el `main.js`, y nada más, así que un `an-image` con `[source]`
  sin esquema no encuentra el fichero. Con una URL `http` sí funciona. El sitio
  donde se arregla es `watchos.rs::assemble`, y le pasa lo mismo a iOS.
- **Animación y transformaciones.** `[animate]`, `translateX`, `scale`,
  `rotate`. En SwiftUI son `withAnimation` y `.offset`/`.scaleEffect`, pero hay
  que decidir dónde vive el estado de la animación: el modelo se reconstruye
  entero en cada foto, y una animación necesita saber de dónde venía. La pila ya
  anima la entrada y la salida de sus pantallas, que es el caso más visible.
- **Bordes por lado y esquinas por esquina.** `borderWidth`, `borderColor` y los
  cuatro radios llegan y hoy se avisan como no leídos.
- **Foto entera vs. mutaciones.** Cada frame con cambios serializa el árbol
  completo a JSON. Para una pantalla de reloj —diez o quince nodos— no se nota,
  y `Codable` ahorra escribir un parser. Cuando aparezca una lista larga habrá
  que mandar solo las ops, y lo que hay que cambiar es `snapshot.rs` y el
  `Decodable` de Swift, no el host.
- **Plugins.** El reloj no los carga: `an-watch` no tiene el registro que tienen
  `an-ios` y `an-android`. `an watchos` lo dice y se para en vez de armar una app
  en la que cada llamada se rechazaría en tiempo de ejecución.
- **Complicaciones y notificaciones.** Son otra superficie del sistema, con su
  propio ciclo de vida; no comparten nada con esto.

## Muros con los que no me topé, y que se esperaban

Se documentan porque estaban en el guion como posibles y conviene saber que no
lo son:

- **El simulador acepta el bundle sin firmar.** `simctl install` no pide firma,
  igual que en iOS. Un dispositivo real sí la pediría.
- **`@main` sobre un `App` de SwiftUI funciona con `swiftc` a pelo**, sin
  `.xcodeproj`, siempre que se pase `-parse-as-library`. Sin ese flag swiftc
  trata el primer fichero como script de nivel superior y `@main` se ignora.
- **El `Info.plist` necesita `WKApplication`.** Es lo que distingue una app de
  watchOS 7 en adelante del viejo par «WatchKit App + WatchKit Extension». Sin
  esa clave `simctl` instala el bundle y luego no encuentra qué lanzar.
- **La medición de texto es la de verdad.** watchOS trae un UIKit recortado sin
  `UIView` pero **con** `UIFont` y el dibujado de cadenas de Foundation, así que
  `WatchMeasurer` mide igual que `UikitMeasurer` en iOS y con la misma tabla de
  pesos. Si divergieran, el mismo texto se mediría distinto en cada plataforma.
