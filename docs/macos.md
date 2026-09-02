# macOS

Angular corriendo como app de escritorio: mismo núcleo, mismo layout, mismo
bundle, y `NSView` de verdad debajo.

```bash
cargo an macos                     # arma el .app y lo abre en esta máquina
cargo an macos examples/kitchen    # con otro ejemplo
cargo an dev --macos               # vigilando, con refresco en caliente
./scripts/check-macos.sh
```

No hace falta nada más: `aarch64-apple-darwin` ya está en cualquier toolchain
estable y el SDK viene con Xcode. No hay simulador que arrancar, no hay aparato
que buscar, no hay `.xcodeproj`.

## Por qué sí es el host de iOS con otras clases

El reloj necesitó un host propio porque watchOS no tiene jerarquía de `UIView`.
Aquí no pasa eso. AppKit es imperativo, `NSView` existe, y el modelo de montaje
del proyecto —crea una vista, métela aquí, cámbiale el marco— encaja tal cual.
Así que `an-macos` es el hermano de `an-ios`: una vista nativa por nodo
montable, el marco escrito directamente porque taffy ya resolvió el layout, y
`MountOp` aplicado sobre la jerarquía.

Lo que se hereda del planteamiento no se repite aquí. Lo que cambia, sí.

## Lo que macOS pinta

Veintidós de las veinticinco primitivas montables van con un control del
sistema o con vistas del sistema. El inventario completo vive en
[`crates/an-macos/src/support.rs`](../crates/an-macos/src/support.rs), en una
tabla, y no repartido por el `match` de `create`: así se puede leer de una vez,
y `scripts/check-macos.py` lo compara con el enum del núcleo para que no se
quede atrás.

| Primitiva | macOS |
|---|---|
| `an-view`, `an-stack-view` | `NSView` |
| `an-text` | `NSTextField` |
| `an-text-input`, `an-search-bar` | `NSTextField`, `NSSearchField` |
| `an-textarea` | `NSTextView` |
| `an-image`, `an-icon` | `NSImageView` (los iconos, SF Symbols por nombre) |
| `an-scroll-view` | `NSScrollView` |
| `an-button` | `NSButton` |
| `an-switch`, `an-slider` | `NSSwitch`, `NSSlider` |
| `an-activity-indicator`, `an-progress-bar` | `NSProgressIndicator` |
| `an-segmented-control`, `an-stepper` | `NSSegmentedControl`, `NSStepper` |
| `an-select`, `an-date-picker` | `NSPopUpButton`, `NSDatePicker` |
| `an-alert` | `NSAlert` |
| `an-web-view` | `WKWebView` |

Dos se arman con vistas del sistema, y se dice cuál es cuál:

- **`an-modal`** es una `NSView` por encima de la raíz. Lo modal de verdad en
  macOS es una hoja (`beginSheet:`) o un `NSPanel`, y las dos sacan el contenido
  fuera de la ventana donde el layout lo colocó. Como el núcleo ya calcula el
  modal a pantalla completa, una capa da el mismo resultado sin poner dos
  sistemas de coordenadas a discutir. El diálogo del sistema —`an-alert`— sí es
  un `NSAlert` de verdad.
- **`an-tab-bar`** es un `NSSegmentedControl`. macOS no tiene barra de pestañas;
  lo que usan las apps de macOS para cambiar de sección es justo un control
  segmentado. `NSTabView` no vale: es la pestaña de documento, con su marco y su
  fondo.

## Lo que macOS no pinta

Tres, y cada una con su motivo escrito en la tabla:

- **`an-navigation-bar`** — la cabecera de un Mac es la barra de título de la
  ventana, que no vive en el árbol de vistas. Poner una barra dentro del
  contenido sería dibujar una segunda cabecera debajo de la de verdad.
- **`an-map-view`** — MapKit existe en macOS, pero `MKMapView` pide clave y
  permisos que este host todavía no gestiona.
- **`an-video-view`** — `AVPlayerView` es de AppKit y no es el
  `AVPlayerViewController` de iOS. No es un port, es otro control.

Ninguna de las tres se imita. Cuando una plantilla las usa, el host lo dice por
la salida de error la primera vez que la monta.

## Las tres cosas del escritorio

### La ventana se redimensiona, y en caliente

En un teléfono el viewport cambia al rotar, que pasa una vez cada mucho. Aquí
cambia mientras alguien arrastra una esquina, sesenta veces por segundo.

`viewDidLayout` llama a `an_runtime_set_viewport` en cada paso del arrastre, y
la función descarta los tamaños repetidos: cada cambio de verdad es una ida y
vuelta al hilo del motor que además espera a que se vacíe lo que hubiera en
vuelo, y eso no se puede pagar por cada `layout()`. El shell llama sin pensarlo
y el filtro está en Rust, que es donde se sabe cuál fue el último.

El reloj de los frames también cambia: el `CADisplayLink` se le pide a la vista
y no a la pantalla. En un Mac hay varias pantallas y pueden ir a refrescos
distintos, así que el bueno es el de la pantalla donde está la ventana, y eso lo
sabe la vista. Va en modo `.common` para que siga latiendo mientras se arrastra
la ventana o hay un menú abierto, que en AppKit corren en un bucle de eventos
aparte.

### Hay ratón, no dedos

Los gestos son los reconocedores de AppKit, que no son los mismos que los de
UIKit. `press`, `doublePress`, `longPress`, `pan`, `pinch` y `rotate` tienen su
reconocedor y se comportan como los de iOS con los umbrales del sistema.

**Deslizar no existe.** AppKit no trae reconocedor de deslizamiento: el gesto de
dos dedos del trackpad llega como scroll, no como gesto. Así que `(swipeLeft)` y
sus tres hermanos **avisan al suscribirse**, no al dispararse. Es la diferencia
entre enterarse al montar la pantalla y quedarse esperando para siempre un
evento que nadie va a mandar.

Por lo mismo avisan `(refresh)` en un `an-scroll-view` —tirar para recargar es
un gesto de dedo; en escritorio se recarga con un botón o con un atajo— y
`(back)` en un `an-stack-view`.

**Hover y cursor no están, y no se disimulan.** El sitio donde tendrían que
empezar es `packages/primitives`: hasta que no haya un `(hover)` que escribir en
una plantilla ni una prop de cursor que poner, montar `NSTrackingArea` en cada
vista sería trabajo por frame que nadie puede pedir. Lo que sí hay es lo que los
controles del sistema hacen ellos solos —un `NSButton` se ilumina al pasar por
encima y un `NSTextField` cambia el puntero a cursor de texto—, porque son
controles de verdad y no dibujos.

### El menú es del sistema

En un teléfono no hay menú. En un Mac lo hay siempre, vive en la barra de arriba
y no dentro de la ventana, así que no cabe en el árbol que el núcleo monta: no
hay `NodeKind` al que corresponda. Lo pone el shell
([`AnMenu.swift`](../shells/macos/Sources/AnMenu.swift)) y no se expone a
Angular.

Que **esté** no es cosmético. Los atajos de edición de macOS —⌘X, ⌘C, ⌘V, ⌘Z,
⌘A— no los implementa `NSTextField`: los reparte el menú por la cadena de
responder. Sin menú de Edición, copiar y pegar en un `an-text-input` no funciona,
sin ningún error y sin nada que mirar. Por eso el menú mínimo incluye Edición y
no solo Salir.

## Cómo se comprueba

Es la única de las cinco plataformas que se puede comprobar de verdad sin
levantar nada: la app corre en la misma máquina que la compiló.
`scripts/check-macos.sh` la arranca, deja que monte el árbol y **le pide una
captura**.

La captura se la hace la app a sí misma —`Screenshot.swift`, con
`cacheDisplay(in:to:)`— y no `screencapture`. Pedirle la pantalla al sistema
exige el permiso de grabación, que se concede a mano y por aplicación: una
comprobación que dependa de eso falla en la máquina de cualquiera que no lo haya
concedido, y falla por algo que no tiene nada que ver con lo que se estaba
comprobando. Una vista sabe dibujarse en un mapa de bits sin pedirle permiso a
nadie, y además pinta los controles de AppKit tal cual están, que es lo que hay
que ver.

Se activa con `AN_SCREENSHOT=<ruta>`; sin esa variable no hay nada de esto en el
camino del frame. Con ella, la app espera a que el núcleo monte, deja pasar unos
frames para que los controles terminen de dibujarse, escribe el PNG y termina
con el código que corresponda. Y cuenta los colores distintos de la imagen: un
PNG del tamaño correcto y enteramente negro pesa lo suyo y pasaría cualquier
prueba que mire el tamaño del fichero.

En ese modo la app **no se pone delante**: una comprobación que te planta una
ventana encima y te come las pulsaciones es una comprobación que nadie deja
correr. Eso se nota en la imagen —los controles salen en gris en vez de con el
color de acento—, y no es un fallo del host: es exactamente lo que macOS pinta
en una ventana que no es la activa. Lanzada a mano, `an macos` sí se pone
delante y los controles salen con su color.

Con `AN_SCREENSHOT_RELOADS=1` la captura no es la del arranque sino la de
**después de una recarga en caliente**, que es la única forma de ver el fallo
que este proyecto ya cometió una vez: una recarga que desmontaba las vistas y
dejaba la ventana en negro. Eso no sale en ningún registro —la app sigue viva,
los frames siguen pasando, nadie devuelve error—; solo se ve mirando la
pantalla. Con `an dev --macos` levantado:

```bash
killall AngularNativeMac
AN_SCREENSHOT=/tmp/hot.png AN_SCREENSHOT_RELOADS=1 \
  build/macos/AngularNativeMac.app/Contents/MacOS/AngularNativeMac
# ahora se guarda un cambio en el ejemplo: la app recarga y se captura sola
```

No está en `check-macos.sh` porque necesita el servidor de desarrollo levantado
y a alguien guardando un fichero, que no es una comprobación sino una sesión de
trabajo. Lo que sí está en el script es el guardián de código: que el `clear()`
del montaje siga detrás de `if !reply.hot`.

## Lo que falta

- **Plugins.** `an-macos` no tiene el registro de módulos nativos que sí tienen
  `an-ios` y `an-android`. `an macos` sobre una app que dependa de un plugin se
  para y lo dice, en vez de armar un `.app` donde cada llamada se rechaza en
  tiempo de ejecución.
- **Las transiciones de `an-native-stack`.** La pila monta y desmonta, pero no
  anima.
- **`an-map-view` y `an-video-view`**, por lo dicho arriba.
- **El campo de contraseña.** `secureTextEntry` llega después de crear el campo,
  y en AppKit `NSSecureTextField` es otra clase: una vista no puede cambiar de
  clase en marcha. Se avisa.
