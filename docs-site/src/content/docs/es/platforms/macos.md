---
title: macOS
description: Angular como app de escritorio — NSView de verdad, una ventana que se redimensiona, un puntero con forma, y la única plataforma que puedes comprobar sin arrancar nada.
sidebar:
  order: 3
---

Angular ejecutándose como app de escritorio: el mismo core, el mismo layout, el
mismo bundle, y `NSView` de verdad debajo.

```bash
an macos                     # monta el .app y lo abre en esta máquina
an macos examples/desktop    # el puntero y el swipe
an macos examples/media      # el mapa y el vídeo
./scripts/check-macos.sh
```

No hace falta nada más: `aarch64-apple-darwin` está en cualquier toolchain
estable y el SDK viene con Xcode. No hay simulador que arrancar, ni dispositivo
que encontrar, ni `.xcodeproj`. Sin argumento de app, `an macos` compila
`examples/controls`. El bundle acaba en `build/macos/AngularNativeMac.app`, y
`--release` y `--no-launch` funcionan los dos.

La app va **firmada ad-hoc** — `codesign --force --sign -` — y ese paso no es
opcional. Sin firmar, macOS mata el proceso en el primer `mmap` de código
generado, que es exactamente lo que hace QuickJS, con un `Killed: 9` y ninguna
explicación.

## Por qué esto *es* el host de iOS con otras clases

El reloj necesitó un host propio porque watchOS no tiene jerarquía de `UIView`.
Aquí eso no pasa. AppKit es imperativo, `NSView` existe, y el modelo de montaje
de este proyecto —crear una vista, ponerla aquí, cambiarle el frame— encaja tal
cual. Así que `an-macos` es el hermano de `an-ios`: una vista nativa por nodo
montable, el frame escrito directamente porque taffy ya resolvió el layout, y
`MountOp` aplicado a la jerarquía.

Aun así es un **crate aparte**, que no comparte código con `an-ios`. Hay dos
tablas duplicadas a sabiendas: la traducción de nombres de SF Symbols, y los
bindings declarados a mano para `WKWebView`, `MKMapView` y `AVPlayerView` —seis
métodos cada uno, en lugar de compilar dos crates generados enteros de framework
en cada build. `objc2` comprueba cada firma contra la real cuando se envía el
mensaje, así que lo que se ahorra en tiempo de compilación no se paga en
seguridad.

El reparto de hilos es el mismo que en iOS: el hilo principal es dueño de las
vistas, QuickJS y el árbol sombra viven en un worker con una pila de 8 MB, y el
presupuesto de fotograma para esperar al motor son 12 ms — deliberadamente no
bajados para pantallas de 120 Hz.

El FFI son seis funciones — `an_runtime_new`, `_eval`, `_reload`,
`_set_viewport`, `_frame`, `_free`. La cabecera dice lo que es: la del teléfono,
menos los plugins.

## Qué dibuja macOS

Los veintiséis primitivos montables. Veintidós son un control del sistema, dos
están ensamblados con vistas del sistema, uno —la cabecera de navegación— macOS
lo pone donde lo guarda, fuera del árbol de vistas, y uno no le toca construirlo
a este crate en absoluto: `an-custom` es el agujero por el que un **plugin**
monta su propia `NSView`. El inventario vive en `crates/an-macos/src/support.rs`, como una tabla en lugar de
disperso por el match de `create`, para poder leerlo de una vez;
`scripts/check-macos.py` lo compara con el enum del core para que no pueda
quedarse atrás.

| Primitivo | macOS |
|---|---|
| `an-view`, `an-stack-view` | `NSView` (`AnFlippedView`) |
| `an-text` | `NSTextField`, configurado como etiqueta |
| `an-text-input`, `an-search-bar` | `NSTextField`, `NSSearchField` |
| `an-textarea` | `NSTextView` |
| `an-image`, `an-icon` | `NSImageView` — los iconos son SF Symbols, por nombre |
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

Dos están ensamblados, y cuáles son se dice en voz alta:

- **`an-modal`** es un `NSView` por encima de la raíz. Lo que de verdad es modal
  en macOS es una hoja (`beginSheet:`) o un `NSPanel`, y los dos sacan el
  contenido de la ventana donde lo puso el layout. Como el core ya maqueta el
  modal a pantalla completa, una capa da el mismo resultado sin dos sistemas de
  coordenadas discutiendo. El diálogo del sistema —`an-alert`— *sí* es un
  `NSAlert` real.
- **`an-tab-bar`** es un `NSSegmentedControl`. macOS no tiene barra de pestañas;
  un control segmentado es lo que las apps de Mac usan de verdad para cambiar de
  sección. `NSTabView` no vale: eso es la pestaña de documento, con su propio
  marco y su fondo.

El mapa y el vídeo salieron aquí más baratos que en el teléfono, por el mismo
motivo: en AppKit son vistas. `MKMapView` hereda de `NSView` y `AVPlayerView`
también, así que se unen al árbol como cualquier otra cosa. UIKit no tiene una
*vista* de vídeo — tiene un `AVPlayerViewController` — y meter eso en el árbol
significa hacerlo hijo del controlador que preside y recolocar su vista a mano
en cada fotograma, porque nace después de fijar el frame.

**El mapa nativo no necesita clave.** Quien la pide es MapKit JS, que es otro
producto. Lo que sí necesita permiso es `[showsUser]`, declarado en el
`Info.plist` del `.app` como `NSLocationUsageDescription`: sin esa clave el
sistema deniega la ubicación por su cuenta, el punto no aparece nunca, y no hay
ningún error que mirar. `check-macos.py` no deja pasar uno sin el otro.

El match de `create` **no tiene comodín**: añadir un `NodeKind` al core rompe
esta compilación en lugar de montar nada en silencio.

## La cabecera es la barra de título

`an-navigation-bar` no se dibuja dentro del contenido: en un Mac la cabecera de
la pantalla en la que estás vive arriba, en la barra de título de la ventana, y
pintar otra debajo serían dos. Lo que no se tira es lo que escribió la
plantilla. El `[title]` acaba siendo el título de la ventana, que es donde lo
busca un usuario de Mac:

```html
<an-navigation-bar [title]="'Notas'" />
```

y la ventana pasa a ser «Notas». Cuando esa pantalla se desmonta, la ventana
recupera el título que tenía. Se aplica desde `flush` y no desde el setter de la
propiedad, porque en ese momento puede que la vista todavía no esté en ninguna
ventana.

El nodo mide **cero por cero**, y eso está escrito en lugar de salir de que
nadie lo mida: una cabecera que no se dibuja pero reserva cuarenta y cuatro
puntos deja una franja vacía bajo la barra de título de verdad, y quien la vea
no tendrá ni idea de dónde salió.

Lo que la barra de título no tiene es botón de atrás. `[showsBack]` y
`[backTitle]` se descartan por eso, y `(back)` avisa al suscribirse: en un Mac se
vuelve atrás con el menú o con un botón propio de la app, no con una flecha en
la cabecera.

Por eso el inventario tiene dos categorías más junto a «control del sistema»,
«ensamblado con vistas del sistema» y «macOS no lo tiene». `Elsewhere` —se
honra, pero no con una vista— es la que usa la cabecera, y es la única que la
usa. `Plugin` es `an-custom`: una `NSView` de verdad, pero de una clase de la
que este crate no ha oído hablar nunca, así que no es ni un `Native` (el nombre
sería mentira) ni un `Assembled` (no se ensambla nada — la vista llega hecha).
Ningún primitivo está `Missing`. Si alguien declara alguno alguna vez, `check-macos.py` lo dice y pide
que se vuelva a escribir el camino del aviso: un nodo que se monta como una caja
vacía en silencio es exactamente lo que este repositorio no permite.

## Las tres cosas de escritorio

### La ventana se redimensiona, y lo hace en vivo

En un teléfono el viewport cambia al rotar, que pasa de vez en cuando. Aquí
cambia mientras alguien arrastra una esquina, sesenta veces por segundo.

`viewDidLayout` llama a `an_runtime_set_viewport` en cada paso del arrastre, y la
función descarta los tamaños repetidos: cada cambio real es un viaje de ida y
vuelta al hilo del motor que además espera a lo que hubiera en vuelo, y eso no se
puede pagar en cada `layout()`. El shell llama sin pensarlo y el filtro está en
Rust, que es donde se conoce el último.

El reloj del fotograma también cambia: el `CADisplayLink` se le pide a la
**vista**, no a la pantalla. Un Mac tiene varias pantallas y pueden refrescar a
distintas frecuencias, así que la buena es la de la pantalla donde está la
ventana, y quien sabe eso es la vista. Corre en modo `.common` para seguir
latiendo mientras se arrastra la ventana o hay un menú abierto, que en AppKit
corren en un bucle de eventos aparte. Pedirle un display link a una vista es
además lo que fija el objetivo de despliegue en macOS 14.

La ventana es de 720×820, con título, cerrable, minimizable y **redimensionable**,
con su marco autoguardado para que el tamaño sobreviva a un relanzamiento. No se
fija nunca un tamaño mínimo. Hay exactamente una ventana, y la app se cierra
cuando esa se cierra: nada expone una segunda.

### Hay un ratón, no dedos

Los gestos son los recognisers de AppKit, que no son los de UIKit. `press`,
`doublePress`, `longPress`, `pan`, `pinch` y `rotation` tienen uno cada uno y se
comportan como los de iOS, con los umbrales del sistema. El pinch y la rotación
informan de una `velocity` de cero, porque el trackpad no la da.

**El swipe sí existe, aunque no es un recogniser.** AppKit no tiene
`NSSwipeGestureRecognizer`. El gesto está; lo que falta es algo que colgar de
una vista. Un swipe llega como un evento suelto, `swipeWithEvent:`, que sube por
la cadena de respondedores como una pulsación de tecla. El sistema lo produce a
partir de lo que el usuario haya configurado en Trackpad, con su umbral y su
número de dedos, así que el juicio de cuándo cuenta sigue siendo de la
plataforma.

Cazarlo exige que el método esté en la clase, y por eso vive con
`AnFlippedView` —la vista que este host monta para `an-view`, `an-stack-view`,
`an-modal` y el documento de una scroll view— y no con los demás gestos. Un
`NSButton` es del sistema y no le puedes añadir un método con la app en marcha,
así que un `(swipeLeft)` puesto directamente sobre un control avisa al
suscribirse y dice dónde ponerlo en su lugar. Eso no es un hueco: un
`swipeWithEvent:` que un control no maneja sube al siguiente respondedor, que es
su vista padre, así que un swipe sobre un botón llega al `<an-view>` que lo
envuelve. Y una vista nuestra que no está escuchando esa dirección **también lo
pasa** en lugar de tragárselo.

Qué dirección es cada signo sale de `NSEvent.h` y no de una suposición: `deltaX`
−1 es hacia la derecha y 1 hacia la izquierda; `deltaY` −1 es hacia abajo y 1
hacia arriba. Vive fuera de la parte que toca AppKit para poder probarlo sin
trackpad, y hay un test que lo fija.

**El hover y el cursor están aquí, y son los del sistema.** Los dos viven en
`NativeVisual`, así que en los veinticinco primitivos:

```html
<an-view [cursor]="'pointer'" (hover)="over.set($event.hovered)">
```

- `(hover)` entrega `{ hovered, x, y }`. Una sola salida con un booleano en
  lugar de dos, porque lo que hay debajo también es una cosa: un
  `NSTrackingArea` da la entrada y la salida por el mismo camino.
- `[cursor]` acepta siete nombres de CSS — `default`, `pointer`, `text`,
  `crosshair`, `grab`, `grabbing`, `not-allowed` — y cada uno es un `NSCursor`
  del sistema. Ninguno está dibujado. Un nombre no reconocido avisa una vez y
  deja el puntero en paz. Los cursores de redimensionado no están: los que macOS
  ha tenido siempre están obsoletos, y sus reemplazos llegaron en macOS 15, más
  tarde que el mínimo de este host.

Los dos se montan sobre un `NSTrackingArea` y no sobre la vista, y ahí está el
truco: **el dueño de un tracking area no tiene por qué ser la vista**. Con un
objeto dueño aparte, `(hover)` y `[cursor]` funcionan sobre un `NSButton` del
sistema exactamente igual que sobre uno nuestro, sin subclasear nada. Poner el
cursor por la otra vía —`addCursorRect:cursor:`— habría exigido sobreescribir
`resetCursorRects`, que es precisamente lo que no puedes hacerle a un control
del sistema.

El área lleva `InVisibleRect`, que es lo que hace innecesario reconstruirla en
cada layout: AppKit la mantiene pegada al rectángulo de la vista. Sin eso, un
área se quedaría del tamaño que tenía la vista al suscribirse, y redimensionar
la ventana —que en un escritorio pasa constantemente— haría que el puntero
entrara y saliera donde no hay nada.

Y lleva `ActiveInActiveApp`, no `ActiveAlways`: en un Mac los controles solo se
iluminan al pasar por encima cuando la app está delante, y este host no va a ser
la excepción que se comporta distinto del resto del escritorio.

Nada de esto le quita a los controles del sistema lo que hacen por sí solos —un
`NSButton` se resalta al pasar por encima y un `NSTextField` convierte el
puntero en una barra de texto— porque son controles de verdad y no dibujos.

Ni `(hover)` ni `[cursor]` llegan a iOS ni a Android, y eso no es un hueco: un
dedo no tiene forma. `check-wrapper.sh` los declara como tales y exige que el
host de escritorio *sí* los lea. Mira
[Props y el envoltorio nativo](/es/guide/native-wrapper/).

### El menú es el del sistema

En un teléfono no hay menú. En un Mac siempre lo hay, vive en la barra de arriba
y no dentro de la ventana, así que no cabe en el árbol que monta el core: no hay
`NodeKind` para él. Lo pone el shell y no se expone a Angular.

Que **exista** no es cosmético. Los atajos de edición de macOS —⌘X, ⌘C, ⌘V, ⌘Z,
⌘A— no los implementa `NSTextField`: los despacha el menú por la cadena de
respondedores. Sin un menú Edición, copiar y pegar en un `an-text-input` no
funciona, sin error y sin nada que mirar. Por eso el menú mínimo incluye Edición
y no solo Salir.

## Los cuatro puntos que una etiqueta se guarda

El layout mide el texto con `boundingRectWithSize:`, que mide **el texto** y nada
más. Un `NSTextField` dibuja ese texto dentro de su celda, y la celda se guarda
unos puntos a cada lado. Hoy son cuatro.

Cuatro puntos suenan a nada y son exactamente el peor tamaño de error. La
etiqueta tiene `wraps` puesto, así que lo que no cabe se va a la línea
siguiente, y esa línea está fuera de la altura que el layout reservó para una.
El texto no se recorta: sale **vacío**, sin error, sin rastro, sin nada que
mirar. Aparece en el momento en que una etiqueta cae en una caja de su tamaño
exacto — es decir, en cualquier `align-items: center`, que es donde apareció.

La medida se le pide al sistema al arrancar, en el hilo principal, junto al
tamaño natural de todos los demás controles, en lugar de escribirla: es una
medida de AppKit y cambia con la versión y con los ajustes de accesibilidad,
exactamente igual que la altura de un `NSSwitch`. El texto en sí se mide con el
`boundingRectWithSize:` de `NSAttributedString` —no el de `NSString`, que en
AppKit no tiene las opciones de fragmento de línea que usa el host de iOS— con
los pesos CSS 100..900 mapeados sobre la escala -1..1 de `NSFont`, cacheado por
texto, fuente y ancho disponible. La cursiva no se resuelve en el medidor:
`NSFont` no tiene fábrica de cursiva, va por un trait de descriptor, y eso es
más trabajo del que compensa por ahora.

## Accesibilidad

Las seis props de accesibilidad aterrizan en el protocolo `NSAccessibility`, y
la historia entera —incluido el filo más afilado de esta plataforma, que
sobreescribir *cualquier cosa* le cuesta a la vista el rol que AppKit le estaba
calculando, así que el primitivo vuelve a escribir a mano el rol implícito— está
en [Accesibilidad en Apple](/es/accessibility/apple/).

Dos props no tienen forma en AppKit y se rechazan en lugar de aproximarse: el
rol `summary`, que es una idea de VoiceOver-en-iOS sin contraparte en
`NSAccessibility`, y el estado `busy`, cuyo pariente más cercano es el *rol*
`AXBusyIndicator` — otra vista, no un estado de esta.

Leer el árbol desde fuera necesita el permiso de Accesibilidad concedido a mano;
el ejemplo es `examples/a11y`.

## Módulos nativos

`device` está registrado, así que `Device.info()` resuelve aquí. Tres de los
cinco campos significan exactamente lo que significan en un teléfono; dos no, y
en lugar de devolver el número que más se le parece, la diferencia queda
escrita:

| Campo | En macOS |
|---|---|
| `platform` | `'macos'` |
| `systemVersion` | `NSProcessInfo.operatingSystemVersion`, como `mayor.menor.parche`. No `operatingSystemVersionString`, que dice «Version 15.3.1 (Build 24D70)» y es prosa. |
| `model` | El identificador de hardware de `sysctl hw.model` — `Mac15,7`, `MacBookPro18,3`. AppKit no tiene `UIDevice.model`, que en un teléfono responde una *clase* de dispositivo; aquí esa palabra sería «Mac» para todos los Mac jamás fabricados. |
| `scale` | El `backingScaleFactor` de la pantalla en la que la app **arrancó**. Una ventana se puede arrastrar a una pantalla con otro factor y esto no lo seguirá: se lee una vez, en el hilo principal, porque `NSScreen` no se puede tocar desde el hilo del motor. Sin ninguna pantalla es `0.0`, la misma respuesta que da visionOS, en lugar de un `2.0` inventado. |
| `locale` | `NSLocale.currentLocale`, BCP 47. |

No hay un segundo módulo: todo lo demás sería un plugin, y mira más arriba.

## Cómo se comprueba

Es la única plataforma que se puede comprobar de verdad sin arrancar nada
externo: la app corre en la misma máquina que la compiló.
`scripts/check-macos.sh` la arranca, la deja montar el árbol y **le pide una
captura**, sobre `examples/controls`, `examples/desktop` y `examples/media`.

Esa misma propiedad es la que hace posible `scripts/check-dev-macos.sh`, y es el
único sitio donde el bucle de desarrollo se comprueba de punta a punta: el
servidor sirve, el `.app` se compila con la URL dentro, la app conecta, se
guarda un fichero y después se le pregunta a la ventana qué está mostrando. Lo
que tiene que estar mostrando es la plantilla nueva *y* el estado de antes de
guardar — `hello-angular` deja en pantalla un temporizador en marcha y un `@if`
que se desplegó a los tres segundos, y un reinicio devuelve los dos a nada.

El modo captura es solo de macOS y lo gobiernan variables de entorno leídas al
arrancar: `AN_SCREENSHOT=<ruta>` más `AN_SCREENSHOT_FRAMES` (40 por defecto),
`AN_SCREENSHOT_PRESS=x,y`, `AN_SCREENSHOT_SWIPE=x,y,dx,dy`,
`AN_SCREENSHOT_HOVER=x,y`, `AN_SCREENSHOT_WINDOW=1`, `AN_SCREENSHOT_RELOADS=n` y
`AN_DUMP_TEXT=1`, que registra las cadenas que las `NSView` montadas están
mostrando de verdad — la única cosa que no se le puede preguntar a un PNG. Las
tres entradas sintéticas disparan dentro de la espera en un orden fijo —la
pulsación a un cuarto de los fotogramas, el swipe a un tercio, el hover a la
mitad— y la captura se toma al final. Usa `cacheDisplay(in:to:)`, que no
necesita permiso de grabación de pantalla, y `CGWarpMouseCursorPosition`, que no
necesita permiso de accesibilidad. Si nunca se monta nada se rinde a los 600
fotogramas en lugar de colgarse.

Una consecuencia de `ActiveInActiveApp`: la aserción del hover se **salta**, y
lo dice, cuando no se puede traer la app al frente. Un salto se informa y no es
un aprobado.

## Llevarlo a otro Mac

```bash
an macos . --sign                # Developer ID, hardened runtime
an macos . --notarize --dmg      # enviado, grapado y empaquetado
```

La firma ad-hoc de más arriba es lo que deja correr la app **aquí**. En
cualquier otro sitio Gatekeeper quiere una firma de Developer ID y un tique de
notarización, y el hardened runtime que exige la notarización no es un cambio de
flag: prohíbe mapear memoria ejecutable y escribible, que es lo primero que hace
el motor de JS. Así que la compilación firmada declara
`com.apple.security.cs.allow-jit`. Sin eso la app muere al arrancar con un
`Killed: 9` que no menciona ningún entitlement — el mismo fallo que la firma
ad-hoc existe para evitar, con la única cara que nadie reconoce.

El `.dmg` se firma y se notariza por derecho propio, porque la imagen es el
fichero que se descarga. Empaquetar uno sí se comprueba aquí; firmar y notarizar
necesitan un certificado de Developer ID de pago y no se han ejecutado nunca.
Mira [Firma y distribución](/es/guide/signing-and-distribution/).

## Lo que falta

- **`(scroll)` no se entrega.** Es un nombre de evento conocido e iOS lo
  implementa; este host no, así que suscribirse a él recibe el aviso genérico de
  «no puedo entregarlo». Es un hueco real, no una decisión.
- **Una vista de plugin no se mide por su contenido.** `<an-custom>` monta lo
  que un plugin registró con `AnPluginViews.register`, llenando la caja que el
  layout le dio al nodo — y un nodo sin tamaño sale a cero, lo que parece un
  plugin que no funciona. Un nombre que nadie registró no monta nada y lo dice
  una vez, nombrando el nombre. Mira
  [una vista que trae un plugin](/es/extending/plugins/#una-vista-que-trae-un-plugin).
- **Los entitlements de un plugin necesitan una firma real.** El host carga
  plugins, pero un `.app` firmado ad-hoc que lleve un entitlement respaldado por
  un perfil lo mata el sistema al lanzarse, así que una compilación sin firmar
  deja fuera esas claves y avisa nombrando la clave, el plugin y el flag que las
  devuelve. Mira
  [Plugins en el Mac y en el reloj](/es/extending/plugins-on-the-mac-and-the-watch/).
- **No hay `an add macos`.** No hay `Info.plist` por proyecto ni id de bundle
  por proyecto: el plist del shell se copia tal cual, y el id es
  `dev.angularnative.playground.mac`.
- **Los radios de esquina desiguales colapsan.** Una `CALayer` tiene un solo
  radio, así que las cuatro esquinas redondeadas toman el mayor de ellos, con un
  aviso una vez.
- **`enabled` en cualquier cosa que no sea un `NSControl`** no hace nada: AppKit
  no tiene `userInteractionEnabled`.
- **`animateDelay` se funde en la duración** —`NSAnimationContext` no tiene
  retardo— y `animateEasing` solo se aplica para `ease-in-out`.
- **`color` en `an-switch`, `an-activity-indicator` y `an-progress-bar`** es el
  color de acento del sistema y no se puede fijar por vista.
- **`resizeMode: 'cover'` encaja dentro en lugar de recortar**, porque
  `NSImageView` no puede recortar.
- **`sheet` en `an-alert`** solo cambia el estilo de la alerta: macOS no tiene
  hoja de acciones. La alerta se presenta con `beginSheetModalForWindow:` y
  nunca con `runModal`, que congelaría el bucle de eventos que mueve el
  fotograma.
- **`(dismiss)` en `an-modal`, `(refresh)` en `an-scroll-view` y `(back)` tanto
  en la stack view como en la barra de navegación** avisan todos al suscribirse,
  cada uno con su motivo.
- **`safeArea` responde una vez con los cuatro insets a cero**, a propósito.
- Props aceptadas y descartadas con un aviso único: `keyboardType`,
  `returnKeyType`, `autoCapitalize`, `autoCorrect`, `secureTextEntry`,
  `thumbColor`, `maximumTrackColor`, `refreshing`, `bounces`, `transition`,
  `showsBack`, `backTitle`, `presentation` y `unselectedColor`. Cualquier cosa
  fuera de esa lista imprime «unknown prop», que las comprobaciones tratan como
  un fallo.
