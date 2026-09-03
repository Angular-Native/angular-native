---
title: tvOS
description: Angular en el Apple TV — el mismo host que iOS, y una plataforma donde no se toca nada.
sidebar:
  order: 4
---

Angular en el Apple TV: mismo núcleo, mismo layout, mismo bundle y —esto es lo
que lo separa del reloj— el **mismo host**. tvOS trae UIKit con jerarquía de
`UIView` y marcos absolutos, que es exactamente el modelo de montaje de este
proyecto, así que `an-ios` compila para la tele sin reescribir nada.

```bash
cargo an tvos                      # examples/hello-tv en el simulador
cargo an tvos examples/controls    # los controles del sistema, para ver los huecos
cargo an dev --tvos                # lo mismo, vigilando y con refresco en caliente
./scripts/check-tvos.sh
```

El refresco en caliente funciona igual que en el teléfono: la app se conecta al
servidor por WebSocket, y al guardar un cambio en un componente el rótulo cambia
en la tele sin reiniciar y con el estado —el contador de segundos— donde
estaba.

Lo que sí cambia, y no es cosmético, es **cómo se maneja**: no hay toques.

## El reparto por familias

`ios.rs` arma el `.app` de las tres familias de UIKit —iOS, tvOS y visionOS—.
Lo que separa a la tele del teléfono cabe en un enum, `Family`, y es poco:

| | iOS | tvOS |
|---|---|---|
| Target de Rust | `aarch64-apple-ios-sim` (nivel 2) | `aarch64-apple-tvos-sim` (**nivel 3**) |
| Triple de swiftc | `arm64-apple-ios17.0-simulator` | `arm64-apple-tvos17.0-simulator` |
| SDK de `xcrun` | `iphonesimulator` | `appletvsimulator` |
| `Info.plist` | `shells/ios/Resources` | `shells/tvos/Resources` |
| Sufijo del `.app` | — | `TV` / `.tv` |
| `std` de Rust | viene hecha | se construye con `-Z build-std` |

No hay un `tvos.rs`. Sería una segunda copia de la misma invocación de `swiftc`,
y la copia es lo que se queda atrás el día que alguien arregla algo en una sola.
Repartir aquí, además, es lo que hace que tvOS herede gratis lo que ese módulo
ya sabía hacer con proyectos de fuera del monorepo: `workspace.build_dir()`, el
`Info.plist` que aporta el proyecto y la comprobación de que ese plist dice lo
mismo que el proyecto.

**Las fuentes Swift son las mismas** (`shells/ios/Sources`). Lo único que no se
comparte es el `Info.plist`, porque las claves que pide cada familia no se
parecen: el de tvOS lleva `UIDeviceFamily = 3` y no lleva `LSRequiresIPhoneOS`,
`UILaunchScreen` ni orientaciones, que son del teléfono.

La tercera familia, visionOS, está en [visionOS](/es/platforms/visionos/).

**El nombre y el identificador llevan sufijo.** `AngularNativeTV` y
`dev.angularnative.playground.tv`. Sin él, `an tvos` pisaría el `.app` que acaba
de dejar `an ios`, e instalar una desinstalaría la otra. `an add tvos` escribe
el `Info.plist` del proyecto con el mismo sufijo, sacado del mismo sitio
(`Family::suffix`), para que los dos no puedan separarse.

### El toolchain, que es la parte que puede no estar

`aarch64-apple-tvos-sim` es un target de **nivel 3**: rustup lo lista pero no
trae `std` precompilada. Hay que construirla en el momento, y eso pide nightly,
igual que ya pasaba con watchOS:

```bash
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly
```

`rquickjs-sys` tampoco envía bindings pregenerados para tvOS; se generan con
bindgen, y tvOS solo tuvo que estar en la lista de `an-bridge/Cargo.toml`.
**QuickJS compila y corre en la tele sin tocar nada.**

Tener el SDK no basta para *ejecutar*: el runtime del simulador es otra descarga
(`xcodebuild -downloadPlatform tvOS`). Cuando falta, `an tvos` arma el `.app` y
lo dice con esas palabras, en vez de decir que no encuentra el dispositivo, que
mandaría a buscar en el sitio equivocado.

## El foco, que es la plataforma

**En una tele no se toca nada.** El mando mueve un cursor invisible entre las
vistas que se declaran enfocables, y el botón central pulsa la que esté enfocada
en ese momento. Ese recorrido lo decide UIKit por geometría, con los marcos de
las vistas — y nuestros marcos son absolutos y los calcula taffy, así que el
motor de foco recibe exactamente la retícula que describe la plantilla y no hay
nada que traducir.

Lo que sí hay que hacer es **declararse**. `-[UIView canBecomeFocused]` devuelve
`NO` de fábrica, y una vista que devuelve `NO` no se puede pulsar en una tele:
el reconocedor se engancha, no falla nada, y el botón sencillamente no responde
nunca. `canBecomeFocused` solo se puede cambiar heredando, así que el host crea
`an-view` en tvOS como una subclase propia, `AnFocusableView`
(`crates/an-ios/src/focus.rs`):

```text
  an-view  ──▶  AnFocusableView : UIView
                 canBecomeFocused = ¿tengo algún reconocedor encima?
                 didUpdateFocusInContext: ──▶ focus / blur hacia el core
```

La respuesta se calcula en el momento en vez de guardarse en un contador: la
vista es enfocable si tiene algún gesto encima, y de esa lista ya lleva la
cuenta UIKit. Así, cuando el core quita el último oyente, la vista deja de ser
enfocable sola.

`an-text` y `an-image` son `UILabel` y `UIImageView` y **siguen sin poder
enfocarse**. En una tele hay que envolverlos en un `an-view`; `events::attach`
lo dice en el log en cuanto alguien le pone un `(press)` a algo que el mando no
puede alcanzar.

### El botón central no es un toque

`UIGestureRecognizer.allowedPressTypes` decide a qué botón del mando responde un
reconocedor, y su valor de fábrica lo documenta el SDK como «platform
dependent». Se fija a mano: `(press)` y `(longPress)` piden `UIPressType.Select`
—el botón central—, y `(back)` pide `UIPressType.Menu`, que es el «atrás» del
mando y lo que en el teléfono es el arrastre desde el borde izquierdo. Los
botones de dirección no se piden nunca: son del motor de foco, y quitárselos
dejaría el mando sin poder moverse por la pantalla.

**Y un `an-button` no responde a `TouchUpInside`.** Este fue el fallo que costó
verlo con el botón en pantalla: `(press)` sobre un `an-button` estaba enganchado
a `UIControlEvents::TouchUpInside`, que es lo que UIKit manda cuando un dedo se
levanta de la pantalla. En una tele no hay dedos: el mando pulsa el botón
central sobre lo enfocado y UIKit manda `PrimaryActionTriggered`. Nada fallaba
—el par target-action se instalaba, el botón tomaba el foco, se ponía blanco y
se levantaba como hace tvOS— y pulsarlo no hacía absolutamente nada. En tvOS se
usa `PrimaryActionTriggered`; iOS se queda con `TouchUpInside`.

### El resalte lo pinta cada control, no el sistema

tvOS **no tiene `UIFocusEffect`** —está marcado `API_UNAVAILABLE(tvos)`, es de
iOS— y el sistema no pinta nada por su cuenta sobre una vista normal. En tvOS el
resalte es cosa de cada control: un `UIButton` se levanta, se pone blanco y
proyecta sombra porque lo dibuja UIKit.

Un `an-view` enfocado, en cambio, **no se ve distinto**. Aquí no se dibuja un
borde ni una escala a mano: sería justo la imitación que este proyecto no hace.
Por eso `examples/hello-tv` lleva dos `an-button` y un `an-view`: mover el foco
entre los dos botones es lo único que una captura puede enseñar, y el contador
propio del `an-view` es lo que demuestra que el foco también llegó a él.

## Qué no existe en tvOS

Esta lista está escrita a mano en
`crates/an-ios/src/family.rs`, y con motivo:
`objc2-ui-kit` genera los enlaces de todas las plataformas Apple sin mirar la
anotación de disponibilidad del SDK, así que `UISwitch::new(mtm)` **compila**
para tvOS y lo que falla es la búsqueda de la clase en tiempo de ejecución, ya
dentro del simulador y con el proceso abortando. La lista es la anotación del
SDK traída al Rust, y es lo que separa «no se puede» de «se cierra sin decir por
qué».

| Primitiva | Qué pasa en tvOS |
|---|---|
| `an-switch` | `UISwitch` no está en el SDK. En una tele un interruptor es una fila enfocable que se pulsa; no hay control del sistema equivalente. |
| `an-slider` | `UISlider` no está. Lo más cercano que sí trae es `UIProgressView`, que solo enseña un valor: no se arrastra. |
| `an-stepper` | `UIStepper` no está. |
| `an-date-picker` | `UIDatePicker` no está: el sistema pide las fechas con una pantalla propia, no con un control que quepa en un marco. |
| `an-web-view` | WebKit entero está fuera del SDK de tvOS. El módulo `web` sale del binario por `cfg`, y no solo por la clase: su `#[link(name = "WebKit")]` haría que el enlazado buscase un framework que no está. |

Cuando el árbol pide una de estas, el nodo **no se crea**, queda un hueco del
tamaño que dijo el layout, y se dice una vez por tipo:

```
angular-native: Switch no está disponible en tvOS: UISwitch no existe en tvOS. …
```

Y lo que tampoco hay, del lado de los eventos:

| Evento | Qué pasa en tvOS |
|---|---|
| `(pinch)`, `(rotation)` | La superficie del mando es de un solo toque, y `UIPinchGestureRecognizer` y `UIRotationGestureRecognizer` no están en el SDK. Se avisa y no se engancha nada. |
| `(refresh)` | `UIRefreshControl` no está, y en una tele no hay de dónde tirar. |
| `(back)` | Sí llega, pero por el **botón de menú** del mando, no por el arrastre desde el borde: `UIScreenEdgePanGestureRecognizer` está `API_UNAVAILABLE(tvos)`. |
| `[sheet]` de `an-modal` | No hay hoja: `UISheetPresentationController` no está y la presentación cubre la pantalla entera, que es como se presenta en una tele. Se dice. |

`(pan)` y los cuatro `(swipe)` **sí siguen funcionando**: la superficie del mando
manda toques indirectos y UIKit los reconoce igual que los del dedo.

Y lo que sí está, y funciona igual que en el teléfono: `an-text`, `an-image`,
`an-scroll-view`, `an-text-input`, `an-textarea`, `an-button`, `an-tab-bar`,
`an-segmented-control`, `an-search-bar`, `an-select`, `an-navigation-bar`,
`an-icon` (SF Symbols), `an-activity-indicator`, `an-progress-bar`, `an-alert`,
`an-modal`, `an-map-view`, `an-video-view`.

## Las medidas son de tele

El viewport son **1920x1080 puntos**, no los 393 de un iPhone. Un `fontSize` de
28 aquí no se lee desde el sofá; `examples/hello-tv` usa 76 para el título y 30
para el cuerpo.

Los bordes de un televisor se recortan —*overscan*— y Apple pide dejar **90
puntos a los lados y 60 arriba y abajo**. Eso no lo pone el framework: lo pone
la plantilla, con `paddingHorizontal` y `paddingVertical`, igual que cualquier
otro margen. `an-safe-area` no ayuda aquí: los `safeAreaInsets` de tvOS son cero.

## Mover el mando desde fuera

`xcrun simctl` **no tiene ningún verbo para el mando**. Está `io … screenshot`,
`io … recordVideo`, `ui`, `spawn`, `push`… y ninguno manda una pulsación; el
binario tampoco tiene nada parecido escondido. Lo que sí hay es la entrada de
teclado del propio Simulator: el simulador de tvOS traduce las flechas a
movimientos del foco y el retorno al botón central.

`scripts/tv-remote.sh` es eso, y nada más: activa Simulator, comprueba que ha
llegado al frente de verdad, y manda los códigos de tecla con `osascript`. En
un simulador de tvOS no hay nada que activar antes: `I/O ▸ Input ▸ Send
Keyboard Input to Device` aparece deshabilitado, porque el teclado va al mando
sin más.

```bash
./scripts/tv-remote.sh down down select    # dos abajo y pulsar
./scripts/tv-remote.sh menu                # el «atrás» del mando
```

Tiene tres condiciones que no se pueden esquivar y que el script comprueba
antes de mandar nada, porque si no la pulsación se va a otro sitio y no se
entera nadie:

- **La pantalla del Mac no puede estar bloqueada.** Con la sesión bloqueada
  ninguna aplicación se puede traer al frente y las teclas no llegan a ningún
  sitio. `CGSSessionScreenIsLocked` lo dice, y el script se para ahí.
- **Simulator tiene que quedarse en primer plano.** Es una limitación real de
  este camino: mientras el script corre, el teclado es suyo.
- **La ventana del Apple TV tiene que ser la que tiene el foco dentro de
  Simulator.** Con un iPhone abierto a la vez, las teclas se las lleva la
  ventana que estuviera delante —que es otro simulador— y en el del Apple TV no
  pasa absolutamente nada. El script levanta la ventana por su título y
  comprueba que se quedó con el foco; `AN_TV_WINDOW` cambia con qué la busca.

## Qué falta

- **`(focus)` y `(blur)` no se pueden pedir desde una plantilla.**
  `AnFocusableView` los emite hacia el core y `events::attach` sabe
  registrarlos, pero en `packages/primitives` esas dos salidas solo existen en
  la directiva de `an-text-input`, no en la base que usan `an-view` y compañía.
  Hasta que estén ahí, una plantilla no puede reaccionar al foco, que es la
  forma natural de resaltar una vista propia en una tele. Es una línea en
  `NativeView`, y es lo primero que hay que hacer aquí.
- **Icono.** El `Info.plist` de tvOS no lleva `CFBundleIcons` a propósito: el
  icono de una app de tvOS es un icono en capas más la imagen de la balda
  superior, y las dos viven en un catálogo de assets compilado con `actool`.
  Aquí no hay ninguno todavía, así que la clave se queda fuera en vez de apuntar
  a un nombre que no existe. La app se instala y se lanza; en la parrilla sale
  sin icono.
- **`an-switch` y `an-slider` no tienen sustituto.** Hoy dejan un hueco y un
  aviso. Lo que corresponde en una tele es una fila enfocable que se pulsa y una
  fila que responde a izquierda/derecha, pero eso es una primitiva nueva y una
  decisión de vocabulario, no un `cfg`.
- **Los plugins se compilan con sus fuentes de iOS**, que es lo único que
  declaran. Si alguna usa API que tvOS no tiene, el enlazado se para con el
  error de swiftc; `an tvos` lo avisa antes de empezar para que no llegue de
  sorpresa. Ningún plugin declara todavía una parte de tvOS aparte.
- **Dispositivo de verdad.** Todo lo de aquí está visto en el simulador de tvOS
  26.5. Un Apple TV físico pediría firma, y el mando de verdad tiene una
  superficie táctil que el simulador no reproduce.
