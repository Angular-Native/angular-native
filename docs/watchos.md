# watchOS

Angular corriendo en el reloj: mismo núcleo, mismo layout, mismo bundle. Lo que
cambia es quién pinta.

```bash
cargo an watchos examples/hello-watch
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
coordenadas que da taffy.

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
alguien le falte un toolchain no tiene por qué tumbar las otras 58
comprobaciones.

`rquickjs-sys` no envía bindings pregenerados para watchOS, igual que no los
envía para iOS ni Android. Se generan con bindgen, que es lo que ya hacía el
resto: watchOS solo tuvo que entrar en la misma lista de `an-bridge/Cargo.toml`.

**QuickJS compila y corre en el reloj sin tocar nada.** Era el riesgo grande del
port y no se materializó.

## Qué hay hecho

Lo que pinta hoy, que es el subconjunto del primer hito:

| Primitiva | Estado |
|---|---|
| `View` | fondo, esquinas, opacidad, `(press)` |
| `Text` | fuente, peso, color, alineación, medido con el `UIFont` real |
| `Button` | rótulo, color, fondo, `(press)` |
| `ScrollView` | desplazamiento con la corona, `contentSize` de taffy |

La medición de texto es la de verdad: watchOS trae un UIKit recortado sin
`UIView` pero **con** `UIFont` y el dibujado de cadenas de Foundation, así que
`WatchMeasurer` mide igual que `UikitMeasurer` en iOS y con la misma tabla de
pesos. Si divergieran, el mismo texto se mediría distinto en cada plataforma.

Los colores se analizan una sola vez, en `an-core::color`. La tabla vivía en
`an-ios`; con un tercer host, dos copias eran dos sitios donde `#0b1020` podía
dejar de ser el mismo azul.

## Qué falta para tener watchOS completo

Nada de esto está empezado; se apunta para que no haya que redescubrirlo.

- **El resto de primitivas.** `Image`, `TextInput`, `Switch`, `Slider`,
  `ActivityIndicator`, `ProgressBar`. Todas caben en el mismo patrón —un `case`
  más en `AnNodeView`— salvo `TextInput`, que en el reloj no es un campo sino
  una pantalla aparte del sistema (dictado, garabateo o teclado), y por tanto no
  es una vista con marco sino una presentación modal.
- **Los que no tienen sentido en el reloj.** `TabBar`, `SegmentedControl`,
  `DatePicker`, `WebView`, `MapView`, `VideoView`. Hay que decidir si se dibujan
  con lo que watchOS sí trae o si se declaran no soportados; hoy salen como una
  caja con el rótulo `Unsupported`, que al menos deja ver dónde está el hueco.
- **La corona digital como fuente de eventos.** Es el gesto propio del reloj y
  ahora mismo solo mueve el `ScrollView`. Debería poder alimentar un `Slider` o
  un `Stepper` como `crown` o similar.
- **Gestos.** Solo hay `press`. `longPress` y `swipe` los da SwiftUI sin
  esfuerzo; `pan`, `pinch` y `rotation` no tienen sentido en una pantalla de
  40 mm.
- **Animación y transformaciones.** `[animate]`, `translateX`, `scale`,
  `rotate`. En SwiftUI son `withAnimation` y `.offset`/`.scaleEffect`, pero hay
  que decidir dónde se guarda el estado de la animación: el modelo se reconstruye
  entero en cada foto, y una animación necesita saber de dónde venía.
- **Recarga en caliente.** El `Reload` del worker ya existe y el host tiene
  `clear()`; falta el `DevClient` del shell, que en iOS es un fichero de 60
  líneas, y meter watchOS en `an dev`.
- **`an dev --watchos`.** Hoy solo hay `an watchos`, que compila y lanza una vez.
- **Foto entera vs. mutaciones.** Cada frame con cambios serializa el árbol
  completo a JSON. Para una pantalla de reloj —diez o quince nodos— eso no se
  nota, y `Codable` ahorra escribir un parser. Cuando aparezca una lista larga
  habrá que mandar solo las ops, y lo que hay que cambiar es `snapshot.rs` y el
  `Decodable` de Swift, no el host.
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
