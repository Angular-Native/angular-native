---
title: Android
description: Los 25 primitivos sobre android.view, Material 3 resuelto sin Gradle, y los seis controles que miden cero hasta que les das un tamaño.
sidebar:
  order: 2
---

Los veinticinco primitivos, sobre `android.view.View` de verdad, con Material 3
debajo y sin Gradle por ninguna parte.

```bash
an android                      # examples/hello-angular en un dispositivo conectado
an android examples/controls    # los controles del sistema
an dev --android                # lo mismo, vigilando, con recarga en caliente
```

El SDK de Android y el NDK se encuentran a través de `ANDROID_HOME`,
`ANDROID_SDK_ROOT` o su ubicación habitual — y `ANDROID_NDK_HOME` para un NDK
que no esté bajo `<sdk>/ndk`. De ellos no hay nada commiteado: la versión y el
nombre del host (`darwin-x86_64`, `linux-x86_64`) se leen del disco, y el
linker, el archivador y el sysroot de bindgen le llegan a `cargo` como entorno.
`.cargo/config.toml` llevaba todo eso escrito hasta hace poco, lo que
significaba que una compilación de Android funcionaba en la máquina de quien
escribió el fichero y en ninguna otra.

Material 3 se resuelve una vez:

```bash
python3 scripts/fetch-android-deps.py
python3 scripts/prepare-android-deps.py
```

## Cómo está montado

El reparto del trabajo es deliberadamente distinto al de iOS. Allí Rust habla
con UIKit directamente, porque el puente con Objective-C es barato y tipado.
Aquí cada llamada cruza JNI, así que la superficie se mantiene lo más pequeña
posible: Java expone un puñado de métodos en una sola clase host y Rust llama a
esos. El crate de Rust son unas mil líneas; solo el host de Java son tres mil
quinientas.

Todo el renderer son doce firmas de método —crear, destruir, insertar, quitar,
poner prop, poner texto, poner listener, poner layout, poner tamaño de
contenido, poner raíz, flush, limpiar— y **cada prop viaja como cadena**, los
colores incluidos. El número de tipos distintos no justifica una firma JNI por
tipo.

Rust guarda el `JavaVM` y nunca un `JNIEnv`, reenganchándose en cada llamada:
todo corre en el hilo de UI, así que sale barato. Una excepción de Java se
describe y se limpia antes de informar de ella, porque si no lo único que
obtienes es «Java exception was thrown» sin saber cuál.

**stderr se redirige a logcat** con una tubería, un `dup2` y un hilo lector. Sin
eso, cada `eprintln!` y cada mensaje de panic se esfuma, y un fallo parece una
pantalla en blanco.

### Dos hilos, en espejo con iOS

La misma pila de 8 MB por el mismo motivo, el mismo presupuesto de 12 ms por
fotograma, la misma regla de no encolar un tick encima de otro que sigue en
marcha, la misma semántica de recarga en caliente. El reloj es un
`Choreographer.FrameCallback` reprogramado en cada fotograma, que es la
contraparte del `CADisplayLink`.

### El modelo de montaje, y el contenedor que no mide nada

Una jerarquía de vistas real: una vista por nodo, `addView` para la estructura,
y frames absolutos convertidos de puntos a píxeles.

El contenedor es un `ViewGroup` propio que **no calcula nada**. Dejar medir a
Android pondría dos motores de layout en la misma pelea que empezaría Auto
Layout en iOS. Sus layout params extienden `MarginLayoutParams` y no los
simples, porque un `ScrollView` es internamente un `FrameLayout` y mide a sus
hijos con `measureChildWithMargins` — con los params simples la app reventaba en
el primer scroll. `onMeasure` mide cada hijo exactamente en su frame e informa
de la unión, porque un `ScrollView` mide a su hijo con una altura sin
especificar y devolver el mínimo sugerido colapsaría el contenido a nada.

Los hijos se recortan por defecto: están posicionados en absoluto y pueden caer
muy fuera del padre.

## Material, y lo que de verdad está dibujado a mano

La mayoría de los controles son los de verdad — `MaterialSwitch`, el `Slider` de
Material, `CircularProgressIndicator`, `LinearProgressIndicator`,
`MaterialButton`, `SearchView`, `Spinner`, `Toolbar`, `WebView`, `VideoView`,
`EditText`, y diálogos reales para `an-alert` y `an-modal`.

Tres que parecen ensamblados no lo son:

| Primitivo | Qué es en realidad |
|---|---|
| `an-tab-bar` | Un `BottomNavigationView`. La píldora tras el icono seleccionado, la animación del cambio, el comportamiento con TalkBack y la altura por versión vienen todos de Material. Solo la reconstrucción del menú y la lista de estados de dos colores están escritas aquí. La propia plataforma de Android no tiene barra inferior — `android.widget` se quedó en las pestañas de 2011. |
| `an-segmented-control` | Un `MaterialButtonToggleGroup`, el botón segmentado de Material 3 tal cual viene. Las formas de los extremos, el contenedor seleccionado, la marca de verificación y la respuesta a la pulsación son de la librería. |
| `an-stepper` | Compuesto, no dibujado: dos botones de icono de Material y un text view de Material. Material 3 no tiene stepper — no es que falte en la librería, es que está ausente del sistema de diseño — así que se ensambla con piezas que *sí* son Material en lugar de dibujar una imitación. Muestra el valor, al contrario que el `UIStepper` de iOS, porque en Android dos botones sueltos no dicen qué cambian. |

Dos cosas sí están dibujadas a mano, y las dos lo dicen:

- **`an-map-view` son teselas de OpenStreetMap sobre un `Canvas`.** Android no
  trae ningún mapa en la plataforma; el de Google vive en los Play Services
  detrás de una clave de API y una dependencia de Gradle. Así que: teselas de
  256 píxeles de `tile.openstreetmap.org`, un descargador de cuatro hilos, una
  caché LRU dimensionada a un cuarto de la memoria de la app, la proyección de
  Mercator escrita a mano, y el arrastre manejado en `onTouchEvent`. Un bitmap
  centinela de un píxel marca una tesela que ya está en vuelo, para que
  arrastrar no la vuelva a pedir. Es una vista nativa de verdad —no un navegador
  escondido— pero tampoco es el mapa del sistema: sin rutas, sin búsqueda, sin
  punto azul.
- **Tirar para refrescar.** El scroll view detecta el arrastre hacia abajo
  estando arriba y dibuja él mismo el arco del spinner, porque
  `SwipeRefreshLayout` es una dependencia aparte de AndroidX. El umbral son
  72 dp.

**Los iconos vienen de una fuente empaquetada**, no de drawables del sistema:
Material Symbols como TTF más su mapa de codepoints, buscados por nombre.
`android.R.drawable` lleva congelado desde 2011 por compatibilidad y no es el
conjunto de Material 3. Donde el sistema quiere un `Drawable` en lugar de una
vista, el glifo se renderiza sobre un bitmap de 24 dp.

En un teléfono no hay nada sin soportar. La maquinaria que rechaza un primitivo
existe y solo se consulta en un reloj — mira [Wear OS](/es/platforms/wearos/).

## El botón atrás

Android tiene dos y el shell responde a los dos, porque el `minSdkVersion` es 24
y el segundo llegó en la 33.

De la API 24 a la 32 es `onBackPressed`, deprecado desde la 33 y aun así el
único atrás que tienen esos niveles. Desde la 33 es un `OnBackInvokedCallback`
en el `OnBackInvokedDispatcher` de la activity, al que el manifiesto se apunta
con `android:enableOnBackInvokedCallback`. Apuntarse no es un adorno: la API 36
quitó la forma de no hacerlo, así que una app que nunca se apuntó no recibe
ninguno de los dos callbacks y el atrás deja de funcionar entero. Medido en un
teléfono con Android 16: atrás en una pantalla apilada salía de la app en lugar
de desapilarla.

Por cualquiera de los dos caminos se le pregunta al host, que envía `back` al
**último** nodo que se suscribió —lo alto de la pila—. El host solo informa;
deshacer la navegación es cosa del router. Ese es el mismo contrato que el gesto
de borde de iOS.

**El atrás predictivo** es la mitad de API 34 del callback,
`OnBackAnimationCallback`. El gesto dice por dónde va y el host coloca las dos
pantallas donde las dejaría el pop en esa fracción: la de arriba saliendo hacia
la derecha y la de debajo volviendo desde el tercio de ancho en el que descansa.
Soltar continúa ese único movimiento en lugar de empezar un segundo; soltar
antes de tiempo las devuelve a su sitio.

El callback se registra **solo mientras hay alguien escuchando**. Uno que se
queda puesto le dice al sistema que la app va a atender todos los atrás, y
entonces el sistema no dibuja ninguna previsualización propia: una app sin pila
en pantalla perdería la animación de vuelta al inicio que tienen todas las
demás.

De que el host informe en lugar de decidir se sigue una cosa: una pila sigue
escuchando también en su propia raíz, así que atrás en la primera pantalla llega
al router, no encuentra a dónde volver y no hace nada. No sale de la app.

## Insets

El camino de los insets lee los insets de la ventana raíz, toma juntos las
barras del sistema y el recorte de pantalla, divide por la densidad, compara
contra los cuatro últimos valores, y solo despacha cuando hay un cambio real.

**Cuatro números no caben en un evento posicional**, así que viajan como JSON y
Rust los parsea a mano, sin parser de JSON, precisamente para que el evento que
llega a la plantilla sea idéntico al que envía iOS. La misma API se usa una
segunda vez para sumar la altura de la franja de gestos a la tab bar medida.

**El teclado va en el inset de abajo** y no en un evento propio, y cómo llega
ahí depende del nivel. Desde la API 30 es `WindowInsets.Type.ime()`, leído una
vez por fotograma de la propia animación del teclado. Por debajo de la 30 no
existe ni ese tipo ni el callback de animación, así que el manifiesto pide
`adjustResize` y el host mide en su lugar: en cada pasada de layout compara el
borde inferior del contenedor con el marco visible de la ventana e informa del
solape. Cuando la ventana sí se redimensionó, el solape es cero —el contenedor
ya termina donde empieza el teclado y el listener del viewport ya rehizo el
layout para el tamaño menor—, así que informar además de la altura del teclado
movería cada formulario dos veces.

## Material 3 sin Gradle

Dos scripts de Python hacen de resolvedor de dependencias.

El primero descarga Material y todo lo que arrastra, resolviendo los POM a mano
contra los repositorios de Google y de Maven Central. Sigue
`dependencyManagement` incluidas las importaciones de BOM —sin eso, androidx
media acaba sin versión y no se descarga nunca— y resuelve los rangos de versión
de Maven a su cota inferior. Antes de eso, cualquier cosa con corchetes en la
versión se descartaba, lo que dejaba fuera media androidx en silencio hasta que
la app arrancaba y no encontraba una clase. Los conflictos se resuelven al
primero visto, el más cercano a la raíz, como hace Gradle. Un puñado de
artefactos se excluyen a propósito: desde Kotlin 1.8 los jars partidos de la
biblioteca estándar están dentro del principal, y entregar los dos conjuntos
hace que el dexer se niegue.

El segundo desempaqueta cada `.aar`, recoge los jars, compila los recursos de
cada librería una vez, y escribe tres listas —un classpath, un conjunto de
recursos y una lista de paquetes— reconstruyendo solo lo que falta, porque los
recursos de cincuenta librerías tardan y no cambian nunca.

En el enlazado eso se convierte en `--extra-packages` (una clase `R` por
librería, cuyos ids por tanto no pueden ser constantes), `--non-final-ids` y
`--auto-add-overlay`, ya que los recursos de las librerías se solapan a propósito
y de otro modo sería un error. Los recursos propios del shell van los últimos
para poder sobreescribir.

## El teclado

Una entrada de texto es un `EditText` pelado, sin padding y sin fondo. Lo que
configura el teclado es un entero, y las cuatro props son banderas suyas —tipo
de teclado, capitalización, autocorrección, entrada segura— así que el entero
entero se **recompone desde el estado guardado** cada vez que llega cualquiera
de ellas. Aplicar una sola borraría las otras tres.

La entrada segura gana sobre la variante de teclado, porque un campo enmascarado
con teclado de correo mostraría el texto. Las banderas de capitalización y de
sin-sugerencias se saltan en teclados numéricos. Y fijar el tipo de entrada
reinicia la tipografía a la monoespaciada de contraseña, así que hay que aplicar
la tipografía otra vez justo después.

## Medición del texto

Un `StaticLayout` sobre un paint compartido, tomando como ancho la línea más
ancha y como alto el del layout. Los dos vuelven empaquetados en un solo `long`
como centésimas de punto: dos llamadas JNI por medición costarían el doble para
nada.

**El peso de la fuente son nueve pasos desde la API 28 y dos por debajo.**
`Typeface.create(family, weight, italic)` acepta el número de CSS desde la API
28, así que a partir de ahí 300 y 500 son pesos propios, igual que en los hosts
de Apple. El `minSdkVersion` del shell es 24, y en 24 a 27 la plataforma no
tiene nada que acepte un número: la tipografía lleva negrita o no-negrita y
nada más, con lo que la escala se colapsa en 600 —de 100 a 500 se dibujan como
normal y de 600 a 900 como negrita—. Un diseño que se apoya en 500 para un
titular obtiene medium en cualquier móvil actual y normal en uno con Android 7.

Las dos mitades se colapsan en el mismo sitio porque las dos pasan por el mismo
`typefaceFor`: el peso que mide el `StaticLayout` es el que dibuja el
`TextView`, y en la rama antigua la medición replica la negrita sintética que
`TextView.setTypeface(tf, style)` le pone a una familia sin corte negrita, que
es más ancha que la normal de la que se falsea. `check-android-java.sh`
demuestra la cuenta sin dispositivo: entra por reflexión en el bytecode de
`AnHost` que acaba de producir el APK, con `Build.VERSION.SDK_INT` y `Typeface`
sustituidos por dobles, y comprueba que hay nueve caras en 34 y dos en 24.

La caché vive en Rust y no en Java, con una clave que lleva todo aquello de lo
que depende la respuesta —el texto, la tipografía, el espaciado, la altura de
línea, el límite de ancho—, por el motivo que tienen los hosts de Apple más
uno: aquí cada consulta cruza JNI, que es bastante más caro que un envío de
mensaje de Objective-C. El ancho infinito viaja como `-1`, porque JNI no tiene
tipo opción.

La altura de línea y el espaciado entre letras se miden y no solo se dibujan.
Los dos se aplicaban al `TextView` y ninguno cruzaba JNI, así que el layout
reservaba una caja para un texto sin ellos y el host dibujaba el texto con
ellos: un espaciado positivo se salía de su caja o partía una palabra antes de
tiempo, y una `lineHeight` mayor que la propia de la fuente no la reservaba
nadie, con lo que las líneas se metían encima de lo que viniera detrás. Viajan
como dos flotantes más en la misma llamada. `Paint.setLetterSpacing` quiere
emes donde el core lleva puntos, así que el valor se divide por el tamaño del
texto —la misma división que hace el lado que dibuja, y el motivo por el que
hay que volver a aplicar el espaciado cada vez que cambia el tamaño—. La altura
de línea se convierte en `setLineSpacing(lineHeight - fontHeight, 1f)`, que es
todo lo que hace `setLineHeight`, con el mismo tope en cero que usa el host por
debajo de la API 28, donde no hay altura de línea, solo lo que se le suma a la
que ya trae la fuente. La ausencia de altura de línea viaja como `-1`, igual
que el ancho infinito.

## Lo que falta

- **El teclado no viaja con su animación por debajo de la API 30.** Desde la API
  30 el IME llega como cualquier otro inset, y llega *en movimiento*:
  `WindowInsetsAnimation.Callback` lo da una vez por fotograma, así que el
  formulario viaja con el teclado. Por debajo de la 30 no hay tal callback ni
  existe `WindowInsets.Type.ime()`, así que lo que se redimensiona es la ventana
  —`adjustResize`— y el layout se rehace una vez en cada extremo en lugar de por
  fotograma. El campo sí se aparta; solo que llega de un salto. Mira
  [Insets](#insets).
- **El peso de fuente son dos pasos por debajo de la API 28.**
  `Typeface.create(family, weight, italic)` acepta el número de CSS desde la API
  28; el `minSdkVersion` del shell es 24 y de la 24 a la 27 no hay nada en la
  plataforma que lo acepte, así que de 100 a 500 se dibuja regular y de 600 a
  900 negrita. La medición colapsa en el mismo sitio, así que la caja sigue
  cuadrando con lo dibujado. Mira [Medición del texto](#medición-del-texto).
- **Sin color de iconos de las barras del sistema por debajo de la API 30.**
  Antes de eso no existe `WindowInsetsController`, así que los iconos de la
  barra de estado y la de navegación se quedan con los del tema y una barra
  clara sobre una app oscura puede costar de leer. Se dice una vez en lugar de
  saltárselo en silencio. Mira
  [Qué aspecto tiene la app](#qué-aspecto-tiene-la-app).
- **Tres salidas que declara la directiva base y este host no entrega**:
  `(hover)`, `(crown)` y `(crownIdle)`. Android sí manda eventos de hover —bajo
  un ratón, bajo un stylus, en un Chromebook— pero `[cursor]`, la prop que
  acompaña a `(hover)`, aquí no significa nada, y media pareja que funciona en
  un Chromebook y nunca en un teléfono se prueba una vez y se publica rota. La
  corona es del reloj: `(crown)` y `(crownIdle)` sí llegan en Wear OS y se
  rechazan en un teléfono. Cada una se rechaza al suscribirse, una vez, con su
  motivo — y también una salida puesta en un primitivo que no la informa,
  `(scroll)` en un `<an-view>`, que se contesta nombrando el widget que el nodo
  montó de verdad.
- **Atrás en la raíz de una pila no sale de la app.** La pila se suscribe a
  `back` mientras esté en pantalla, así que el host atiende cada pulsación y el
  router no encuentra luego nada que desapilar. Mira
  [El botón atrás](#el-botón-atrás).
- **`armeabi-v7a` solo si la pides.** `--abi armeabi-v7a` la compila; no está
  en ningún valor por defecto. Ver [Qué ABIs](#qué-abis).

Avisos, y ninguno silencioso: la corona en un teléfono (una vez); un paso de
slider que no divide el rango, lo que hace que Material reviente al dibujar, así
que se informa y el slider se queda continuo; un estado de accesibilidad que no
es un objeto; un `checked` que no es ni booleano ni `'mixed'`; un rol
desconocido, que existe porque llegar hasta él significa que la lista de roles
de TypeScript y la del host se han separado; `expanded` en una vista sin
`(press)`, ya que en Android expandir es una *acción* y sin ella un lector
anunciaría algo que no se puede hacer. Qué se aplica y qué se rechaza está en
[Accesibilidad en Android](/es/accessibility/android/).

## Qué aspecto tiene la app

`app.appearance` en `angular-native.json` acepta `system`, `light` o `dark`, y
`system` es el valor por defecto: teléfono en claro, app en claro.

```json
"app": { "name": "MyApp", "bundleId": "com.example.myapp", "appearance": "system" }
```

`an` lo escribe en el manifiesto fundido como una entrada `<meta-data>` y la
Activity lo lee antes de `super.onCreate` — después, `AppCompatActivity` ya ha
leído el modo nocturno y se recrearía en el primer fotograma. Lo que lo sigue
es Material: diálogos, selectores de fecha, los tiradores de selección de
texto. **No** es lo que pinta la pantalla; eso es el fondo de la propia app, así
que una app que siga al dispositivo tiene que pintar con él.

Esto antes se forzaba. El shell llamaba a `setDefaultNightMode(MODE_NIGHT_YES)`
en un bloque estático, en todas las apps compiladas con él, y el motivo era
real: con el sistema en claro salía una barra de navegación blanca debajo de
una pantalla que la app había pintado oscura. Aquello curaba un síntoma que era
de las barras quitándole la decisión a todo el mundo.

Las barras se resuelven ahora donde viven. La ventana ya dibuja de borde a
borde, así que son transparentes y lo que se ve detrás es el fondo de la propia
app; sus iconos se eligen por la luminancia de ese fondo — la relativa de sRGB,
porque el ojo es unas siete veces más sensible al verde que al azul y un azul
saturado que promedia «claro» se lee como oscuro. Por debajo de la API 30 no
hay `WindowInsetsController` y los iconos se quedan los del tema, lo cual se
dice una vez en lugar de saltárselo en silencio.

## A Google Play

```bash
an android --sign --release      # un APK firmado para publicar
an android --aab --release       # el bundle que acepta Play
```

El keystore de depuración que usa la compilación de esta página es el que genera
Android Studio, con la contraseña escrita en el código; no lo acepta ninguna
tienda. `--sign` usa un keystore que generas y guardas tú, `--aab` compila un
Android App Bundle — que es lo único que acepta Google Play desde agosto de
2021.

### Qué ABIs

```bash
an android                                  # arm64-v8a
an android --abi x86_64                     # un emulador en una máquina Intel
an android --abi arm64-v8a,x86_64           # las dos, en un mismo APK
an android --aab --release                  # arm64-v8a y x86_64
```

Un APK lleva `arm64-v8a` y nada más. Es un solo fichero que tiene que contener
todas las arquitecturas en las que pueda llegar a instalarse, y cada una es otra
compilación cruzada del core con QuickJS dentro — un coste que el bucle de
desarrollo paga entre guardar un fichero y verlo, para un dispositivo cuya
arquitectura no cambia.

Un bundle lleva `arm64-v8a` **y** `x86_64`. Play parte un bundle por ABI e
instala un trozo, así que el segundo destino no le cuesta un byte a nadie en la
descarga; dejarlo fuera le cuesta a la ficha todos los dispositivos x86-64 que
existen — los Chromebooks, y el emulador de cualquier máquina Intel, que es lo
que está ejecutando quien revisa o prueba la app desde un escritorio. Play no
avisa de esto. Sencillamente la app no se ofrece ahí.

`armeabi-v7a` no está en ninguno de los dos valores por defecto. Los
dispositivos que solo son de 32 bits son menos del 1% de los que están en uso, y
lo que Play exige es que *exista* una compilación de 64 bits, no que exista una
de 32 — así que sería un coste en todas las compilaciones para un público que
casi nadie tiene. `--abi armeabi-v7a` está ahí para quien sí.

`--abi` sustituye al valor por defecto en vez de sumarse a él, así que
`an android --aab --abi arm64-v8a` es la forma de obtener un bundle con una sola
ABI. Cada ABI necesita su destino de Rust instalado; `an` dice cuáles faltan
antes de compilar nada, en un solo mensaje, en vez de pararse a mitad de la
segunda.

Para esto tampoco hay Gradle. `aapt2 link --proto-format` produce el manifiesto
y los recursos en protobuf que quiere un bundle, el módulo se ensambla a mano,
`bundletool` lo convierte en el `.aab` y `jarsigner` lo firma, porque `apksigner`
se niega. `bundletool` no forma parte del SDK de Android —
`scripts/fetch-android-deps.py` lo trae.

Todo este camino se ejecuta de punta a punta en `scripts/check-signing.sh`, con
un keystore que genera la propia comprobación: una clave de subida no necesita
el permiso de nadie. Lo que esa comprobación no puede hacer es subir nada, y
`an` tampoco. Qué conseguir de Google, y dónde poner el keystore, está en
[Firma y distribución](/es/guide/signing-and-distribution/).

## Compilar y ejecutar

```text
cargo build --target aarch64-linux-android -p an-android
aapt2 compile / link  ·  javac  ·  d8  ·  zip  ·  zipalign  ·  apksigner
→ build/android/<AppName>.apk
```

`javac` corre con `-source`/`-target 17` en lugar de `--release`, porque con
`--release` ignora el boot classpath y la compilación tiene que ir contra
`android.jar`. El dexer parte en varios ficheros dex en cuanto Material está
dentro, y todos van al zip. De `aapt2` solo sale el manifiesto, así que los dex,
la librería compartida y los assets se meten en el zip después.

**La firma es solo de depuración**: el keystore de depuración estándar, creado
con `keytool` si no está. Aquí no hay identidad de publicación.

El id de la app puede diferir del paquete del shell: el paquete del manifiesto
se renombra en el enlazado mientras las clases se quedan donde están, y por eso
al lanzar se cualifica la activity como
`<applicationId>/dev.angularnative.MainActivity`.

**`${applicationId}` en el manifiesto se sustituye**, y es el único marcador que
hay. Renombrar el paquete reescribe el paquete y cualifica los nombres de clase
relativos; deja el valor de cada atributo exactamente como lo encontró, y hay un
atributo que no sobrevive a eso. La autoridad de un `<provider>` es única en
todo el **dispositivo**, así que el `FileProvider` del shell —por el que `share`
entrega los ficheros— no puede llevar un nombre fijo: dos apps de
angular-native lo reclamarían y la segunda en instalarse fallaría con
`INSTALL_FAILED_CONFLICTING_PROVIDER`. Se escribe `${applicationId}.anfiles` y
sale como el de la propia app.

Las entradas `<uses-permission>` y `<uses-feature>` de un plugin se funden en
una copia del manifiesto, nunca en el tuyo. Un permiso duplicado se salta
—pedirlo dos veces es pedirlo una— y una característica que la app ya declara
con un `required` distinto conserva la de la app, con un log.

**Un dispositivo físico funciona.** Los dispositivos se listan con `adb` y se
clasifican preguntándole a cada uno sus características de build; se exige
exactamente uno de la forma correcta, y `--device` en `an wearos` se rechaza si
la forma no cuadra. Todas las llamadas a `adb` llevan `-s`: sin eso `adb` se
niega a actuar en cuanto hay dos dispositivos conectados — y si el otro no está
autorizado ni siquiera se niega, le manda el APK del reloj al teléfono.

La recarga en caliente llega a un teléfono real. La dirección que se cocía en
la app era `10.0.2.2` —la máquina anfitriona vista desde dentro del emulador, y
nada en absoluto desde cualquier otro sitio—, así que un teléfono por USB
sondeaba un sitio que no estaba y no decía nada, porque no había fallado nada.
`an dev --android` abre ahora el puerto en el dispositivo con `adb reverse` y
cuece `127.0.0.1` a secas, la misma dirección que usan todos los demás targets.
Un dispositivo que rechace el reverse recibe igualmente la app, y se le dice que
guardar no va a cambiar nada en pantalla.

El cliente de desarrollo hace **long-polling sobre HTTP** en lugar de usar un
WebSocket, porque la plataforma no trae cliente de WebSocket y arrastrar una
librería HTTP entera para esto no compensa.
