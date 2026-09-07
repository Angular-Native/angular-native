---
title: iOS · iPadOS
description: La plataforma de referencia — los 25 primitivos sobre vistas de UIKit de verdad, y el puñado de cosas que UIKit puso más difíciles de lo que parecen.
sidebar:
  order: 1
---

La plataforma contra la que se mide todo lo demás. Los veinticinco primitivos
son vistas reales de UIKit, todos los eventos se entregan, y funcionan tanto los
módulos nativos como los plugins.

```bash
an ios                      # examples/hello-angular en el simulador
an ios examples/controls    # los controles del sistema
an dev                      # lo mismo, vigilando, con recarga en caliente
```

No hace falta nada más: `aarch64-apple-ios-sim` está en cualquier toolchain
estable y el SDK viene con Xcode. **No hay `.xcodeproj`** — un proyecto de Xcode
solo añadiría un fichero de dos mil líneas que nadie puede revisar en un diff.

## Cómo está montado

Un solo crate, `an-ios`, sirve a tres familias: iOS, tvOS y visionOS. Lo que las
separa es un enum y un puñado de `cfg` — mira [tvOS](/es/platforms/tvos/) y
[visionOS](/es/platforms/visionos/) para saber qué pierde o gana cada una. En
iOS no falta nada: la lista de «aquí no está disponible» está vacía.

El shell son cinco ficheros Swift y unos cientos de líneas. `main.swift` llama a
`UIApplicationMain` sin storyboard; el app delegate crea una `UIWindow` con los
límites de la pantalla y un único view controller raíz. En iOS no hay scene
delegate — ese fichero es enteramente de visionOS.

El FFI son seis funciones: `an_runtime_new`, `_eval`, `_reload`,
`_set_viewport`, `_frame`, `_free`. Los plugins añaden cuatro más, y esas son
globales al proceso y no al runtime.

### Dos hilos

El hilo principal es el dueño de cada `UIView` — `an_runtime_new` devuelve nulo
si no se le llama ahí. El motor de JS, el árbol sombra y taffy viven en un
worker con una **pila de 8 MB**, y ese número está medido y no elegido: el
router de Angular necesita algo más de 3 MB para completar una navegación, y a
2 MB la transición avanza siete eventos y se para. El hilo principal de iOS
tiene 1 MB que no puedes cambiar, y por eso el motor no puede vivir ahí.

En cada fotograma el hilo de UI espera **como mucho 12 ms** de los 16,6 ms la
respuesta del worker, luego monta lo que haya llegado y sigue. Si el worker
sigue ocupado no se encola ningún tick nuevo, o la cola crecería sin límite. El
reloj es un `CADisplayLink` en modo `.common`, y su marca de tiempo es el único
reloj de la app: los timers de JS avanzan con los fotogramas, no en un hilo
propio.

Los panics tienen un hook que escribe el sitio en stderr y hace flush a mano,
porque un panic dentro de una función `extern "C"` aborta y un mensaje
almacenado en buffer no se vería nunca.

### El modelo de montaje

Una jerarquía real de UIKit: una vista nativa por nodo, `insertSubview(at:)`
para la estructura, y `setFrame` con el rectángulo que produjo taffy. Auto
Layout no se usa nunca — sería un segundo motor de layout compitiendo con el
primero.

Toda vista creada recibe `translatesAutoresizingMaskIntoConstraints = true`, y
esa línea tiene una historia. Antes era `false`, lo cual era inofensivo hasta
que el primer control con restricciones intrínsecas —una `UISearchBar`, un
`UIDatePicker`— encendió Auto Layout para toda la ventana y puso a cero todos
los frames. El síntoma era la pantalla entera amontonada en una esquina.

Una `UIScrollView` recibe su `contentSize` del core, que lo calcula. Una stack
view recorta a sus límites, pone la pantalla que entra encima independientemente
del índice en el árbol, y retiene la que sale hasta que termina su animación.

## En qué se convierte cada primitivo

El mapeo completo está en [Componentes](/es/reference/components/). Cuatro
decisiones merecen explicación:

- **`an-tab-bar` es un `UITabBarController` entero**, no una `UITabBar` suelta.
  Desde iOS 26 una barra suelta se dibuja a través de su propio proveedor
  visual, y en iPad **aparece dos veces** — una arriba y otra en el frame que le
  dio el layout. Se fuerza también el modo `.tabBar`, porque `.automatic` puede
  convertirla en una barra lateral en iPad.
- **`an-select` es un `UIButton` con `showsMenuAsPrimaryAction`**, no un
  `UIPickerView`. La picker view es la rueda a pantalla completa, que es otra
  cosa.
- **`an-map-view` es un `MKMapView` de verdad.** La clase está declarada a mano,
  porque el crate generado de MapKit solo cubre macOS.
- **`an-alert` y `an-modal` montan un marcador oculto** y presentan un
  controlador real. Un modal es un `UIViewController` y no una vista superpuesta
  porque sin él UIKit no sabe que hay algo modal delante: VoiceOver seguía
  leyendo lo que había detrás, y el orden de presentación frente a un
  `UIAlertController` dependía del orden de creación de las vistas.

Los iconos son SF Symbols pedidos por su nombre. No se empaqueta nada.

Los radios de esquina no uniformes no tienen API en UIKit, así que el contorno
se dibuja como un `UIBezierPath` y se instala como máscara de shape layer — y se
redibuja en cada cambio de tamaño, porque una máscara no se estira.

## El área segura

Suscribirse a `(safeArea)` registra el nodo e informa de inmediato; a partir de
ahí los `safeAreaInsets` del contenedor se releen en cada layout, y solo sale un
evento cuando los cuatro números cambian de verdad.

`<an-safe-area>` los aplica como **padding, no como margen**, y el motivo vale
la pena: una vista que se apartara con un margen dejaría de estar bajo la muesca,
entonces informaría de cero, volvería, y haría eso para siempre.

## Dos batallitas que lleva el código

**Los rasgos de teclado van por el runtime, no por setters generados.**
`[keyboardType]`, `[returnKeyType]`, `[autoCapitalize]` y `[autoCorrect]` se
aplican buscando la implementación con `class_getMethodImplementation`. En iOS
26, `-[UITextField setKeyboardType:]` **no está en la tabla de métodos de la
clase** — UIKit lo resuelve de forma perezosa — así que `respondsToSelector:`
dice que sí, `class_getInstanceMethod` dice que no, la capa de binding se cree
al segundo y aborta. Todas las apps con un campo de texto reventaban al
arrancar.

**El vídeo necesita un bucle de reintento.** Llamar a `play()` sobre un
reproductor que aún no ha cargado deja la velocidad a cero para siempre, sin
error y con una capa negra, así que la llamada se reemite en cada fotograma
hasta que prende. La vista del reproductor también hay que redimensionarla en
`flush`, porque se crea después de fijar el frame.

## Gestos y eventos

Todo lo que hay en [Componentes](/es/reference/components/) se entrega en iOS. El
mecanismo depende del nodo: target-action para los controles, un delegate para
el scroll, gesture recognisers para las vistas.

- `(press)` en un botón es `TouchUpInside`. En una tele no — mira
  [tvOS](/es/platforms/tvos/).
- Los eventos de una barra de búsqueda se enganchan a su campo de texto interno,
  no a la barra.
- `(refresh)` es un `UIRefreshControl` de verdad.
- `(back)` en un `an-stack-view` es un `UIScreenEdgePanGestureRecognizer` en el
  borde izquierdo, y solo dispara en `Ended` — un arrastre cancelado no debe
  navegar. El host solo informa; deshacer la navegación es cosa del router.
- Los toques fuerzan `userInteractionEnabled` a activo, porque `UILabel` y
  `UIImageView` vienen con él apagado.

**En iOS nada avisa al suscribirse.** Un nombre de evento que el host no
reconoce se ignora en silencio, deliberadamente: una plantilla puede llevar un
`(click)` heredado de la web, y eso no es motivo para hacer ruido. Los avisos al
suscribirse son todos de tvOS y visionOS.

## `[sheet]` en un modal

`[sheet]` da un `pageSheet` con tirador y exactamente dos detentes, medio y
grande. **Están fijados en el código** — no hay ninguna prop para elegirlos.
Cualquier otra cosa se presenta como `overFullScreen` con una cobertura
vertical, elegida porque el core ya ha maquetado el contenido a pantalla
completa, así que no hay que recolocar nada.

## Medición del texto

`boundingRectWithSize:` con origen de fragmento de línea e interlineado de la
fuente. Los pesos CSS 100..900 se mapean sobre la escala -1..1 de UIKit en nueve
pasos. `boundingRect` no sabe nada de `lineHeight` ni de `numberOfLines`, así
que el número de líneas se deriva y se remultiplica después, y el resultado se
redondea hacia arriba para que no se recorte el último glifo.

La caché no es una optimización: el layout pide el mínimo de contenido, el
máximo y el tamaño final, por nodo y por fotograma. La clave son el texto, el
tamaño, el peso, la cursiva, la familia, el espaciado entre letras y el ancho
disponible redondeado a un octavo de punto, y se vacía cuando cambian la escala
de la pantalla o el ajuste de tipografía dinámica.

Los tamaños de los controles se miden una vez al arrancar en el hilo principal,
construyendo cada control y preguntando `sizeThatFits`. Se registran once; un
slider, una barra de progreso, una tab bar, una barra de búsqueda y un control
segmentado se estiran al ancho disponible.

## Compilar y ejecutar

```text
cargo build --target aarch64-apple-ios-sim -p an-ios
xcrun swiftc -target arm64-apple-ios17.0-simulator … -lan_ios
→ build/ios/<AppName>.app
```

El plist se comprueba contra `angular-native.json` **antes** de compilar nada:
no merece la pena quemar medio minuto de cargo y swiftc para decir que el nombre
no cuadra. La cobertura de plugins se comprueba lo primero de todo. Las fuentes
Swift —las del shell, las compartidas, las de cada plugin y el registro
generado— van en una sola invocación de `swiftc`, ordenadas, para que un plugin
vea `AnPlugin` sin importar nada y la línea de comandos no cambie con el orden
del sistema de ficheros.

Los entitlements, cuando un plugin los pide, se incrustan **dentro del binario**
con `-sectcreate`, no se ponen en una firma. En el simulador eso es lo que
funciona: `keychain-access-groups` es un entitlement restringido y macOS se
niega a ejecutar un binario que lo lleve en una firma sin un perfil de
aprovisionamiento. El síntoma era que la app no arrancaba, con una denegación
que no menciona los entitlements por ninguna parte. Xcode hace lo mismo para el
simulador.

El lanzamiento es `simctl boot` → abrir Simulator → `bootstatus -b` → terminar →
desinstalar → instalar → lanzar. La espera importa: instalar sobre un simulador
a medio arrancar se cuelga en silencio. La desinstalación también: instalar
encima de una app existente no reemplaza el bundle de forma fiable, y la app
arranca con el código viejo. El dispositivo se encuentra parseando
`simctl list devices available -j` como JSON, porque `simctl` imprime el udid
antes del nombre y hacer grep instala en el simulador equivocado.

**No hay firma**, y **no hay dispositivo físico**. El target de Rust es el del
simulador, el SDK es `iphonesimulator`, y el camino de lanzamiento es `simctl`
de punta a punta. Un dispositivo real necesita la otra ruta —una identidad y un
perfil— y esa no está aquí.

## A un iPhone de verdad, y a la tienda

```bash
an ios --physical                # firmado, instalado con devicectl
an ios --archive                 # .xcarchive y .ipa
```

Las dos son la misma compilación con otro destino: `aarch64-apple-ios` en lugar
del target del simulador, el perfil incrustado en el bundle, y una firma real
sobre todo el conjunto. Los entitlements se mudan también —en el simulador viven
dentro del binario, en un dispositivo viven en la firma— y se toman de lo que
concede el perfil de aprovisionamiento, porque el sistema no le da a una app
nada que su perfil no lleve.

Ninguna de las dos la ha ejecutado nadie de este proyecto contra un dispositivo
físico ni contra una cuenta de Apple Developer. Lo que sí está comprobado es que
se paran antes de compilar cuando falta una credencial, y dicen cuál. Mira
[Firma y distribución](/es/guide/signing-and-distribution/), que es explícita
sobre qué caminos se han ejecutado y cuáles están escritos a partir de la
documentación de Apple.

## iPadOS

No hay un camino de código separado para iPadOS. Es iOS, con tres sitios donde
se reconoce al iPad, y los tres existen porque sin ellos se rompía:

1. **`UIDeviceFamily = [1, 2]` en el plist.** Sin eso la app se declara solo
   para iPhone, un iPad la ejecuta en una ventana de compatibilidad de 320×480
   escalada, y presentar cualquier controlador del sistema hace que iOS quite el
   escalado, con lo que el contenido aparece diminuto en una esquina.
2. **Una hoja de acciones necesita un origen en iPad.** El popover se ancla al
   centro inferior del contenedor cuando el idiom es `.pad`, y deliberadamente
   no se ancla en iPhone, donde eso le daría un pico a la hoja. Sin el ancla
   UIKit no avisa — revienta.
3. **La tab bar.** El controlador, y forzar el modo `.tabBar`, los dicta el
   comportamiento del iPad.

**La multitarea no se maneja explícitamente.** No hay manifiesto de escenas, ni
`UIRequiresFullScreen`, ni manejo de clases de tamaño o de trait collections. El
redimensionado funciona solo de forma genérica: los nuevos límites pasan por
`an_runtime_set_viewport` en cada layout, y `an-safe-area` vuelve a informar en
el siguiente. Las orientaciones soportadas son vertical y las dos horizontales.

## Lo que falta

- **No hay input accessory view.** El teclado en sí está resuelto — va en el
  inset inferior, en movimiento, y `<an-safe-area>` viaja con él; mira
  [El área segura y el teclado](/es/guide/safe-area-and-keyboard/). Lo que no
  hay forma de pedir es la barra de encima: un botón Hecho, una flecha al
  siguiente campo, cualquier cosa para la que sirve `inputAccessoryView`.
- **`letterSpacing` no se mide.** Forma parte de la clave de la caché de
  medición pero no se pasa ningún atributo de kerning, así que el layout
  dimensiona el texto como si fuera cero.
- **`an-navigation-bar` no tiene tamaño natural.** Su tamaño de control no se
  registra nunca, así que mide 0×0 salvo que la plantilla le dé una altura.
- **Los detentes de la hoja no se pueden elegir** (arriba).
- **Seis sitios que avisan una vez, todos en accesibilidad**: un rol
  desconocido, una clave de estado desconocida, un rol para el que UIKit no
  tiene rasgo, `checked: 'mixed'` —UIKit solo conoce marcado y sin marcar, así
  que el valor se deja vacío en lugar de redondearlo— y `expanded` y `busy`, que
  no tienen rasgo ninguno de los dos. Qué se aplica y qué se rechaza está en
  [Accesibilidad en Apple](/es/accessibility/apple/).
