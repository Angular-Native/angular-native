# macOS

Angular corriendo como app de escritorio: mismo núcleo, mismo layout, mismo
bundle, y `NSView` de verdad debajo.

```bash
cargo an macos                     # arma el .app y lo abre en esta máquina
cargo an macos examples/desktop    # el puntero y el deslizamiento
cargo an macos examples/media      # el mapa y el vídeo
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

Las veinticinco primitivas montables. Veintidós van con un control del sistema,
dos se arman con vistas del sistema, y la que queda —la cabecera de
navegación— macOS la pone donde la tiene: fuera del árbol de vistas. El
inventario completo vive en
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
| `an-map-view` | `MKMapView` |
| `an-video-view` | `AVPlayerView` |

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

El mapa y el vídeo salieron más baratos aquí que en el teléfono, y por el mismo
motivo los dos: en AppKit son vistas. `MKMapView` hereda de `NSView` y
`AVPlayerView` también, así que entran en el árbol como cualquier otra. En UIKit
no hay vista de vídeo —hay un `AVPlayerViewController`—, y meterlo en el árbol
obliga a hacerlo hijo del controlador que manda y a recolocarle la vista a mano
en cada frame, porque nace después de que el marco esté puesto. Aquí no hay nada
de eso.

Las clases se declaran a mano, en [`map.rs`](../crates/an-macos/src/map.rs) y
[`video.rs`](../crates/an-macos/src/video.rs), y no se traen de
`objc2-map-kit` ni de `objc2-av-kit`. Es la misma decisión que ya estaba tomada
en [`web.rs`](../crates/an-macos/src/web.rs) para `WKWebView`: por seis métodos
entrarían a compilar dos frameworks enteros de clases generadas en cada build
del host. `objc2` comprueba cada firma contra la de verdad al mandar el mensaje,
así que lo que se ahorra en tiempo de compilación no se paga en seguridad.

**El mapa nativo no pide clave.** La que la pide es MapKit JS, que es otro
producto. Lo que sí pide permiso es `[showsUser]`, y el permiso se declara en
el `Info.plist` del `.app` (`NSLocationUsageDescription`): sin esa clave el
sistema deniega la ubicación él solo, el punto no sale nunca y no hay error que
mirar. `check-macos.py` no deja que una cosa vaya sin la otra.

## La cabecera es la barra de título

`an-navigation-bar` sigue sin dibujarse dentro del contenido, y eso no ha
cambiado: en un Mac la cabecera de la pantalla en la que estás vive arriba, en
la barra de título de la ventana, y pintar otra debajo serían dos. Lo que ha
cambiado es que **ya no se tira lo que la plantilla escribió**. El `[title]`
acaba en el título de la ventana, que es donde un usuario de Mac lo busca:

```html
<an-navigation-bar [title]="'Notas'" />
```

y la ventana pasa a llamarse «Notas». Al desmontarse esa pantalla, la ventana
recupera el título que tenía.

El nodo mide **cero por cero**, y eso está escrito en
[`controls.rs`](../crates/an-macos/src/controls.rs) en vez de salir de que
nadie lo midiera: una cabecera que no se dibuja pero reserva cuarenta y cuatro
puntos deja una franja vacía bajo la barra de título de verdad, y quien la vea
no va a saber de dónde sale.

Lo que la barra de título no tiene es botón de atrás. `[showsBack]` y
`[backTitle]` se descartan con ese motivo, y `(back)` avisa al suscribirse: en
un Mac se vuelve con el menú o con un botón de la app, no con una flecha en la
cabecera.

Por eso el inventario tiene una cuarta categoría además de «control del
sistema», «armada con vistas del sistema» y «macOS no la trae»: `Elsewhere`, que
es «sí se cumple, pero no con una vista». Hoy solo la usa la cabecera. Un
`Missing` deja a la plantilla sin lo que pidió; esto se lo da donde la
plataforma lo tiene.

Con eso **no queda ninguna primitiva sin pintar en este host**, y el camino que
avisaba al montar una ausente se quitó porque ya no lo recorre nadie. Si alguien
vuelve a declarar una como `Missing`, `check-macos.py` lo dice y pide que se
vuelva a escribir: un nodo que se monta como una caja vacía y en silencio es
justo lo que este repositorio no admite.

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

**Deslizar sí existe, aunque no sea un reconocedor.** AppKit no tiene
`NSSwipeGestureRecognizer`, y de ahí venía el aviso de antes. Pero el gesto está:
lo que no hay es un reconocedor que colgarle a una vista. Un deslizamiento llega
como un evento suelto, `swipeWithEvent:`, que sube por la cadena de responder
igual que una pulsación de tecla. Lo produce el sistema a partir del gesto que
el usuario tenga configurado en Trackpad, con su umbral y su número de dedos, o
sea que el criterio de cuándo cuenta sigue siendo el de la plataforma y no el
nuestro.

Recogerlo exige que el método esté en la clase, y por eso vive en
[`flipped.rs`](../crates/an-macos/src/flipped.rs) y no con los demás gestos:
`AnFlippedView` es la vista que monta este host para `an-view`, `an-stack-view`,
`an-modal` y el documento de un `an-scroll-view`. Un `NSButton` es del sistema y
no se le puede añadir un método con la app en marcha, así que un `(swipeLeft)`
puesto directamente en un control avisa al suscribirse y dice dónde ponerlo. No
es un agujero: un `swipeWithEvent:` que un control no atiende sube al siguiente
de la cadena, que es su vista padre, así que un deslizamiento por encima de un
botón acaba llegando al `<an-view>` que lo envuelve. Y una vista nuestra que no
escucha esa dirección **también lo pasa**, en vez de tragárselo.

Qué dirección es cada signo lo dice `NSEvent.h` y no una suposición: `deltaX`
−1 es hacia la derecha y 1 hacia la izquierda; `deltaY` −1 es hacia abajo y 1
hacia arriba. Está en `support::swipe_direction`, fuera de la parte que toca
AppKit, para poder probarlo sin trackpad; y hay una prueba que lo fija.

Lo que sigue avisando al suscribirse es `(refresh)` en un `an-scroll-view`
—tirar para recargar es un gesto de dedo; en escritorio se recarga con un botón
o con un atajo—, `(back)` en un `an-stack-view` y `(back)` en un
`an-navigation-bar`.

**Hover y cursor ya están, y son del sistema.** `packages/primitives` tiene una
salida y una entrada nuevas, las dos en `NativeVisual`, o sea en todas las
primitivas:

```html
<an-view [cursor]="'pointer'" (hover)="encima.set($event.hovered)">
```

- `(hover)` entrega `{ hovered, x, y }`. Es una sola salida con un booleano y no
  dos, porque lo que hay debajo también es uno solo: un `NSTrackingArea` da la
  entrada y la salida por el mismo camino.
- `[cursor]` acepta siete nombres de CSS —`default`, `pointer`, `text`,
  `crosshair`, `grab`, `grabbing`, `not-allowed`— y cada uno es un `NSCursor`
  del sistema. Ninguno se dibuja. No están los de redimensionar: los que macOS
  tiene desde siempre están marcados como obsoletos y los que los sustituyen
  llegaron en macOS 15, que es posterior al mínimo que compila este host.

Las dos se montan sobre un `NSTrackingArea` y no sobre la vista, y ahí está la
gracia: el dueño de un área de seguimiento **no tiene que ser la vista**. Con un
objeto aparte de dueño, `(hover)` y `[cursor]` funcionan igual encima de un
`NSButton` del sistema que encima de una vista nuestra, sin subclasear nada.
Poner el cursor por el otro camino —`addCursorRect:cursor:`— habría exigido
sobrescribir `resetCursorRects`, que es exactamente lo que no se puede hacer con
un control del sistema.

El área va con `InVisibleRect`, que es lo que hace que no haya que rehacerla en
cada `set_layout`: AppKit la lleva pegada al rectángulo de la vista. Sin eso, un
área se quedaría del tamaño que tenía la vista al suscribirse, y al redimensionar
la ventana —que en escritorio pasa constantemente— el puntero entraría y saldría
por donde ya no hay nada.

Y va con `ActiveInActiveApp`, no con `ActiveAlways`: en un Mac los controles solo
se iluminan al pasar por encima cuando la app está delante, y este host no va a
ser la excepción que se comporta distinto que el resto del escritorio. Tiene una
consecuencia para las comprobaciones, y está más abajo.

Nada de esto quita lo que los controles del sistema hacen ellos solos —un
`NSButton` se ilumina al pasar por encima y un `NSTextField` cambia el puntero a
cursor de texto—, porque son controles de verdad y no dibujos.

Ni `(hover)` ni `[cursor]` llegan a iOS ni a Android, y no es un hueco: es que
un dedo no tiene forma. `check-wrapper.sh` lo tiene declarado como tal y le
exige al host de escritorio que sí las mire. Ver
[docs/wrapper-nativo.md](wrapper-nativo.md).

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

## Los cuatro puntos que un rótulo se guarda

El layout mide el texto con `boundingRectWithSize:`, que mide **el texto** y
nada más. Un `NSTextField` dibuja ese texto dentro de su celda, y la celda se
guarda unos puntos a cada lado. Hoy son cuatro.

Cuatro puntos parecen nada y son justo el peor tamaño de error. El rótulo va con
`wraps`, así que lo que no cabe se lleva a la línea siguiente, y esa línea está
fuera del alto que el layout reservó para una. El texto no sale recortado: sale
**vacío**, y no hay error, ni traza, ni nada que mirar. Se ve en cuanto un
rótulo cae en una caja de su tamaño exacto, o sea en cualquier
`align-items: center`, que es donde apareció.

La medida se le pregunta al sistema al arrancar, en el hilo principal, junto al
tamaño de todos los demás controles ([`controls.rs`](../crates/an-macos/src/controls.rs)),
en vez de escribirse: es una medida de AppKit y cambia con la versión y con los
ajustes de accesibilidad, igual que el alto de un `NSSwitch`.

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

### Y se le puede dar de comer

Una captura del arranque enseña la pantalla inicial y nada más. Lo que hay que
ver de un escritorio es lo que pasa **cuando alguien hace algo**, así que el
mismo modo acepta tres cosas más, todas en el entorno y todas fuera del camino
del frame si no se piden:

| Variable | Qué hace |
|---|---|
| `AN_SCREENSHOT_FRAMES=n` | cuántos frames esperar antes de disparar (40 por defecto) |
| `AN_SCREENSHOT_PRESS=x,y` | pulsa el control del sistema que haya bajo ese punto |
| `AN_SCREENSHOT_SWIPE=x,y,dx,dy` | manda un deslizamiento a la vista que haya bajo ese punto |
| `AN_SCREENSHOT_HOVER=x,y` | lleva el puntero ahí antes de disparar |
| `AN_SCREENSHOT_WINDOW=1` | fotografía la ventana entera, barra de título incluida |

Las tres primeras van en ese orden dentro de la espera —a un cuarto, a un
tercio y a la mitad—, así que se pueden pedir juntas.

**El puntero se mueve de verdad.** `CGWarpMouseCursorPosition` no pide ningún
permiso —no es `CGEventPost`, que sí exige accesibilidad—, así que lo que entra
en el `NSTrackingArea` es el ratón y lo que sale en la imagen es el área del
sistema haciendo su trabajo, no un estado puesto a mano. Con dos consecuencias
que hay que decir: esa captura **sí** se pone delante, porque las áreas de este
host son `ActiveInActiveApp` y una comprobación no puede pedirle al host que se
comporte distinto que el resto del escritorio; y el puntero se le devuelve a
quien lo tenía en cuanto la foto está hecha.

**El deslizamiento no se puede provocar de verdad, y eso se dice.** El gesto lo
reconoce el sistema a partir de dos dedos en el trackpad y no hay forma de
pedírselo. Lo que hace la comprobación es entrar por su misma puerta
—`swipeWithEvent:` sobre el resultado de `hitTest:`— con los deltas que manda
él, y comprobar todo lo que viene después: que la vista lo recoge o lo pasa a su
padre, que el evento cruza al motor y que la plantilla se recompone. Que un
gesto de dos dedos acabe en un `swipeWithEvent:` es lo único que hay que mirar
con la mano.

**Del vídeo no se puede fotografiar la imagen.** `AVPlayerView` compone sus
fotogramas fuera del dibujado de la vista —la capa de vídeo viene del
decodificador, no de un contexto de dibujo—, así que `cacheDisplay` coge el
marco y los controles y deja el hueco en negro. No es del host: es de la
captura, y es la misma razón por la que `screencapture` tampoco vale aquí. Lo
que sí se comprueba del vídeo es que se monta con su vista de verdad y que el
reproductor no falla; si falla, el host lo dice, porque un vídeo roto y uno que
todavía está cargando se ven exactamente igual.

Con eso, `check-macos.sh` corre además el ejemplo `examples/desktop` —el
puntero y el deslizamiento— y el ejemplo `examples/media` —el mapa y el
vídeo—, y compara la captura de la ventana quieta con la de la ventana con el
ratón encima: un `(hover)` que llega y no cambia nada en pantalla es la mitad
del trabajo.

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
- **El campo de contraseña.** `secureTextEntry` llega después de crear el campo,
  y en AppKit `NSSecureTextField` es otra clase: una vista no puede cambiar de
  clase en marcha. Se avisa.
- **Los cursores de redimensionar.** Por lo dicho arriba: los viejos están
  obsoletos y los nuevos piden macOS 15.
- **Que un gesto de dos dedos de verdad acabe en `swipeWithEvent:`.** Eso lo
  decide el sistema y solo se puede mirar con la mano; lo de aquí para abajo sí
  está comprobado.
