---
title: Wear OS
description: Angular en un reloj Android — el mismo host que el teléfono, una pantalla redonda, la corona rotatoria, y los siete primitivos que no pintan nada en una esfera.
sidebar:
  order: 7
---

Angular en el reloj de Android: el mismo core, el mismo layout, el mismo bundle
y —esta vez— **el mismo host**.

```bash
an wearos examples/hello-wear   # una vez, en un emulador de reloj
an dev --wearos                 # lo mismo, vigilando, con recarga en caliente
./scripts/check-wearos.sh
```

## Por qué esto no se parece en nada al reloj de Apple

[watchOS](/es/platforms/watchos/) no tiene jerarquía de `UIView`, y por eso
necesitó un host declarativo propio: el árbol de Rust se refleja en un modelo
que SwiftUI redibuja.

Nada de eso aplica aquí. Un reloj Wear OS **es** Android: `Activity`,
`Choreographer`, `android.view.View`, `ViewGroup.addView`, `setBackground`. El
host que ya existía —`crates/an-android` más el shell de Java— funciona tal
cual. Haz grep de `wear`, `crown`, `rotary` o `watch` en
`crates/an-android/src/` y no encuentras nada: no hay Rust específico de Wear
por ninguna parte. El mismo crate, el mismo `aarch64-linux-android`, la misma
`dev.angularnative.MainActivity`.

Y funciona literalmente: lo primero que se probó fue instalar el APK del
teléfono, sin tocar, en un emulador de Wear OS 5. Arrancó, evaluó el bundle y
pintó. Así que la parte cara —QuickJS, el puente binario, taffy, las vistas— no
hubo que hacerla. El trabajo del reloj está en otro sitio, y son cuatro cosas.

## 1. El APK del reloj no es el del teléfono

Aunque el del teléfono arranque. Lo que hace de esto una app de reloj es una
línea del manifiesto:

```xml
<uses-feature android:name="android.hardware.type.watch" android:required="true" />
```

Sin ella, para el sistema y para la tienda esto es una app de teléfono que
resulta que se ha instalado en un reloj. Es el fallo que no se ve: se instala
igual, arranca igual y pinta igual. Te enteras al publicar.

Y como lo que cambia es un fichero entero —`aapt2` no sabe nada de variantes ni
de condicionales— el manifiesto del reloj es un fichero aparte,
`shells/android/AndroidManifest.wear.xml`, y `an wearos` elige cuál enlaza. Un
proyecto propio lo sobreescribe dejando un `AndroidManifest.wear.xml` en su
directorio de plataforma `android/`. Ese directorio lo crea `an add android`,
solo con el manifiesto del teléfono: **`an add wearos` no existe**, porque la
compilación de Wear reutiliza el directorio de plataforma de Android.

El APK recibe además otro nombre de fichero —`<app>-wear.apk`— porque los dos
llevan el mismo paquete y un nombre compartido los pisaría.

El tema también cambia, y por eso salió del manifiesto a
`res/values/styles.xml`: el del reloj necesita `windowSwipeToDismiss`. Wear OS
no tiene botón atrás; se arrastra desde el borde izquierdo. Los temas Wear del
sistema lo tienen, los de Material 3 —que son de teléfono— no, y sin él la app
no tiene salida y no da ningún error: simplemente no pasa nada al deslizar.

`Theme.DeviceDefault`, que es lo que recomienda Google para una app Wear basada
en vistas, no se puede usar: `MainActivity` es una `AppCompatActivity`, y
AppCompat se niega a arrancar con un tema que no descienda de
`Theme.AppCompat`. Así que `Theme.AngularNative.Wear` extiende
`Theme.Material3.Dark.NoActionBar` —Dark fijo en lugar de DayNight, porque la
pantalla es OLED y eso es la mitad de la batería— con fondo de ventana negro.

El host tampoco se fía del manifiesto en tiempo de ejecución. Le pregunta al
sistema —`PackageManager.FEATURE_WATCH`— porque el APK del teléfono se puede
instalar en un reloj, y entonces el manifiesto miente. Que sea redonda es una
pregunta aparte, `Configuration.isScreenRound()`, deliberadamente independiente:
los relojes cuadrados existen.

## 2. La pantalla es redonda

Es lo que más cambia y lo que menos código costó, porque ya había dónde ponerlo.

En una esfera las esquinas no existen. Lo que pongas ahí no se recorta con un
aviso: no se pinta, y nadie dice nada. Esa es exactamente la pregunta que hace
la muesca de un iPhone —«¿hasta dónde puedo pintar?»— y `an-safe-area` ya la
responde en las otras plataformas, así que el margen de la curva viaja por ahí y
no por un evento nuevo. Una plantilla no tiene por qué saber si el margen que le
apartan viene de una muesca o de un arco.

Lo que da el sistema y lo que hay que deducir, que no son lo mismo:

- **El sistema da:** que la pantalla es redonda, y la barbilla de los relojes
  que la tienen, que sí llega como `WindowInsets`.
- **Se deduce:** cuánto hay que apartarse. El lado del cuadrado inscrito en un
  círculo de diámetro `d` es `d/√2`, así que sobra `d·(1 − 1/√2)`, repartido
  entre los dos lados: **0,146447 del diámetro por lado**. Es la misma cifra que
  usa el `BoxInsetLayout` de `androidx.wear`.

`BoxInsetLayout` no se usa directamente, y no por evitar arrastrar la
dependencia: es un `ViewGroup` que posiciona a sus propios hijos, y aquí el
layout es de taffy. Eso serían dos motores de layout decidiendo lo mismo, que es
precisamente lo que evita el host de watchOS.

Los insets se leen de `WindowInsets` —`systemBars() | displayCutout()`— y luego
el inset redondo se `max`-ea contra los cuatro bordes, para que una barbilla y
un arco no se cancelen. El resultado se despacha como el mismo evento `safeArea`
con `{top, right, bottom, left}`.

En un emulador de 454 px a densidad 2 eso son 227 puntos de diámetro y 33,2 de
margen por lado: el cuadrado usable es de 160×160.

## 3. La corona

La corona giratoria de Wear OS **no es un toque**. Llega como `ACTION_SCROLL`
desde `SOURCE_ROTARY_ENCODER`, por el camino de eventos genéricos
—`onGenericMotionEvent`— y no por el de los toques. Por eso el host no la veía:
`AnScrollView` solo miraba `onTouchEvent`.

Tres cosas que no son obvias:

- **El valor está en muescas de rueda, no en píxeles.** `AnScrollView` las
  convierte con `ViewConfiguration.getScaledVerticalScrollFactor()`, el mismo
  factor que usa un ratón, y niega el signo — girar la corona hacia arriba es
  positivo, y desplazar hacia abajo aumenta `scrollY`.
- **Los eventos van a la vista que tiene el foco.** Una lista no lo pide por su
  cuenta: sin `setFocusableInTouchMode(true)` el sistema se los manda a quien
  tenga el foco —normalmente nadie— y la corona no hace nada, sin ningún error.
- **Eso solo se enciende en un reloj.** En un teléfono, una lista que reclama el
  foco se lo quita al campo de texto que hubiera debajo.

Desde la plantilla no se declara nada. `an-scroll-view` la escucha, y `(scroll)`
sale exactamente como si hubiera arrastrado un dedo, así que `an-virtual-list`
—que se apoya en el mismo scroll— la mueve la corona sin tocarla.

### Y en crudo, para lo que no es desplazar

Desplazar es lo que casi siempre quieres, pero no siempre: subir un volumen,
mover una hora, cambiar de pantalla. Eso es `(crown)`, la misma salida que usa
el reloj de Apple, entregada aquí por el host de Android:

```html
<an-view (crown)="turned($event)" (crownIdle)="stopped()"> … </an-view>
```

| Clave | Qué es |
|---|---|
| `delta` | Muescas giradas desde el evento anterior. |
| `offset` | Acumulado desde que empezó este giro. |
| `velocity` | Muescas por segundo, con signo. |

Cuatro diferencias con el reloj de Apple, y las cuatro son de la plataforma:

- **Viajan muescas, no puntos.** Lo que da el sistema es el eje de una rueda de
  ratón; convertirlo a píxeles es cosa de la scroll view, y hacerlo también aquí
  inventaría una escala que la plantilla no pidió.
- **`offset` cuenta desde que empezó el giro**, no desde que la vista tomó el
  foco. En watchOS el foco se ve —hay un resaltado—; en Android una vista que
  toma el foco no cambia de aspecto, así que nadie podría ver ese origen.
- **`(crownIdle)` lo cuenta el host**, porque el sistema no envía ningún final:
  envía muescas y se calla. Un cuarto de segundo de silencio es el corte.
  `velocity` es 0 para la primera muesca de un giro en lugar de un número
  enorme.
- **En un `an-scroll-view` la corona hace las dos cosas.** Al oyente se le
  consulta antes del desplazamiento y `onGenericMotion` devuelve false, así que
  escuchar sin más no congela la lista. `examples/hello-wear` muestra a la vez
  los puntos desplazados y las muescas giradas, del mismo giro.

Fuera de un reloj, `(crown)` lo dice al suscribirse y no dispara nunca: un
teléfono no tiene rueda que girar, y una salida que no dispara nunca sin que
nadie lo diga es peor que no tenerla.

Lo que **no** hace hoy: mover un `an-slider` o un `an-stepper` por sí sola. La
corona llega a la plantilla y la plantilla puede mover el valor; lo que no hay
es un control que la tome del sistema, como hacen el `Slider` y el `Stepper`
enfocados de watchOS.

## 4. Lo que no pinta nada en un reloj

Cuando el host descubre que está en un reloj, siete primitivos no se montan. En
su lugar va un marcador visible con el nombre de la etiqueta, en rojo a 11 dp, y
un error en el log con el motivo.

| Primitivo | Por qué no |
|---|---|
| `an-tab-bar` | Wear OS no tiene barra de pestañas: se navega deslizando y con la corona, no con pestañas abajo. |
| `an-segmented-control` | Un control segmentado no cabe a lo ancho de una esfera. |
| `an-navigation-bar` | Arriba en un reloj está el reloj del sistema, y «atrás» es el deslizamiento de borde. |
| `an-search-bar` | Buscar en un reloj no es un campo dentro de la pantalla sino la pantalla de entrada del sistema — dictado, escritura a mano o teclado. |
| `an-web-view` | Wear OS no trae WebView: ningún paquete del sistema implementa `android.webkit`. |
| `an-date-picker` | El selector de la plataforma es un calendario de teléfono; en un reloj una fecha se elige a pantalla completa. |
| `an-select` | Un desplegable ancla un menú, y una esfera no tiene dónde anclarlo. |

El marcador se guarda lo suyo: su id va a un conjunto que consultan los caminos
de propiedades y de texto, así que no se queda el `[title]` ni el `[color]` del
primitivo al que sustituyó. Si lo hiciera, iría vestido de ese primitivo —es un
`TextView`, así que una etiqueta y un color entrarían directos— y parecería que
el control ha funcionado. `check-wearos.sh` comprueba que la lista de Java y la
de esta página dicen lo mismo.

**Los dos relojes difieren**, y la diferencia conviene saberla si escribes para
los dos: el de Apple soporta `an-select` y `an-date-picker` y no puede con
`an-textarea`, `an-map-view` ni `an-video-view`; Wear excluye `an-select` y
`an-date-picker` y conserva los tres.

Lo que sí funciona, y por qué no está en la lista: `an-switch`, `an-slider`,
`an-button`, `an-progress-bar`, `an-activity-indicator`, `an-icon`,
`an-stepper`, `an-alert` y `an-modal` son controles de Material que se dibujan
igual en cualquier pantalla; `an-text-input` abre el teclado del sistema, que en
un reloj es la pantalla de entrada de Wear; y `an-video-view` y `an-map-view` se
montan exactamente igual que en un teléfono — el mapa ni siquiera es el del
sistema, son teselas de OpenStreetMap sobre un `Canvas`.

De esa lista, `an-switch`, `an-slider`, `an-progress-bar`, `an-icon` y
`an-button` se han visto corriendo en el emulador, con el aspecto de Material
que tienen en un teléfono. `an-video-view` se monta y pide foco de audio, pero
la imagen Wear del emulador no trae códecs —«OMX service is not available»— y el
reproductor acaba en `error (100, 0)`. Eso es el emulador, no el reloj, pero
hasta probarlo en uno de verdad no se puede decir otra cosa.

## Probarlo sin reloj

Por defecto no hay ningún AVD de Wear. Crea uno:

```bash
sdkmanager "system-images;android-34;android-wear;arm64-v8a"
avdmanager create avd -n an-wear \
  -k "system-images;android-34;android-wear;arm64-v8a" -d wearos_large_round
```

Hay dos cosas de su `config.ini` que hay que cambiar — `avdmanager` deja las dos
en «no»:

```ini
hw.lcd.circular=true    # sin esto isScreenRound() es false y no hay inset redondo
hw.rotaryInput=yes      # sin esto no hay corona que girar
```

Arranca el emulador con su puerto fijado. Eso no es una costumbre: `adb`
identifica el dispositivo por él —`emulator-5560`— y con un teléfono en marcha
al mismo tiempo, un `adb` sin `-s` no sabe a cuál te referías.

```bash
emulator -avd an-wear -port 5560 -no-boot-anim
```

La corona se gira desde el terminal, que es como se tomaron las capturas:

```bash
adb -s emulator-5560 shell input rotaryencoder scroll --axis SCROLL,-3   # abajo
adb -s emulator-5560 shell input rotaryencoder scroll --axis SCROLL,3    # arriba
```

El valor está en muescas y una muesca es mucho: con el factor de desplazamiento
del sistema a densidad 2, `SCROLL,-1` mueve unos 43 puntos, más de un cuarto de
los 160 usables. Para ver el desplazamiento desde dentro —en lugar de aterrizar
al final de la lista— sirven las fracciones: `SCROLL,-0.2` son unos 9 puntos.

El emulador Wear vuelve a la esfera a los diez segundos sin tocarlo, como un
reloj de verdad. Para tenerlo quieto mientras miras:

```bash
adb -s emulator-5560 shell settings put system screen_off_timeout 1800000
adb -s emulator-5560 shell svc power stayon true
```

`an wearos` elige el dispositivo preguntándole su forma —`ro.build.characteristics`
contiene `watch` en cualquier imagen de Wear OS— así que con un teléfono y un
reloj en marcha a la vez cada APK va al suyo, y `--device` nombra uno por su
serie de adb. Un `--device` que no sea un reloj se rechaza en lugar de
instalarse: el APK del reloj se instala en un teléfono sin quejarse, que es la
clase de fallo que solo ves al publicar.

## Recarga en caliente

`an dev --wearos` monta el APK del reloj —no el del teléfono mandado a otro
sitio—, lo empuja al dispositivo con forma de reloj, y se queda vigilando. Al
guardar, el bundle nuevo se cose encima del que corre: el cambio se ve sin
perder la pantalla ni el estado. Su `--device` es una serie de `adb`, no un
nombre de simulador.

La dirección es `127.0.0.1` y el puerto se abre en el dispositivo con
`adb reverse`, igual que el del teléfono — así que esto funciona en un reloj
puesto en una muñeca y no solo en el emulador. A diferencia de los shells de Apple, que usan un WebSocket,
el shell de Android hace long-polling contra el servidor de desarrollo.

Lo primero que hizo fue dejar la pantalla en negro, y de ahí salieron dos fallos
que este documento no podría haber encontrado:

- **Un nodo que se movía no se desregistraba de su padre anterior.** Cuando
  `an-safe-area` se convierte en una vista después de que sus hijos ya estén
  montados —que es exactamente lo que pasa cuando el árbol se reconstruye en
  caliente— la eliminación no se enviaba nunca. UIKit y AppKit mueven una vista
  que ya tiene padre sin decir nada, así que nunca se notó en iOS ni en el
  escritorio; `ViewGroup.addView` lanza, y el subárbol se quedaba sin montar. El
  core ya no acepta esa inserción.
- **Tras una recarga, las plantillas se quedaban sin directivas.** La recarga
  vaciaba la lista de directivas de la definición esperando que Angular la
  recalculara, y Angular no la recalcula nunca. Pasó desapercibido porque un
  input de un primitivo que no encaja con nada llega igualmente como propiedad;
  lo que no llega es lo que hace que algo sea un componente en lugar de una
  propiedad, y aquí eso es `an-safe-area`, que dejó de crecer y dejó de
  apartarse del arco. Una esfera es la única pantalla donde ese fallo se ve de
  un vistazo.

Los dos eran fallos de todas las plataformas. El reloj es quien los encontró.

## Lo que falta

- **Una corona que mueva un control.** `(crown)` llega a la plantilla y un valor
  se puede mover a mano, pero un `an-slider` o un `an-stepper` no la toman del
  sistema como hacen los de watchOS, donde basta con tener el foco. Aquí habría
  que llevarles el foco y el eje, y eso es cosa del primitivo.
- **Modo ambiente.** Un reloj de verdad baja a una pantalla en blanco y negro a
  1 Hz cuando la muñeca se baja. Hoy la app simplemente se cierra, que es lo que
  hace Wear OS con una app que no lo declara. No hay `AmbientModeSupport`, ni
  `WearableActivity`, ni dependencia de `androidx.wear` por ninguna parte. Es
  otra superficie del sistema con su propio ciclo de vida.
- **Complicaciones, tiles y esferas.** No comparten nada con esto.
- **`an-text-input`.** Se monta y abre el teclado del sistema, pero no se ha
  verificado que el flujo de entrada de Wear —dictado y escritura a mano—
  devuelva el texto donde el host lo espera.
- **Relojes cuadrados.** Funcionan —el inset redondo solo se aplica si el
  sistema dice que la pantalla es redonda— pero no se han probado.
- **Un reloj de verdad.** Todo lo de aquí se vio en el emulador `an-wear`, una
  imagen de Wear OS 5 de 454 px a densidad 2. Queda por ver en hardware: la
  corona física, el vídeo, que allí no tiene códecs, y la batería, que en una
  pantalla OLED es la mitad del diseño.

La accesibilidad en un reloj es el contrato de accesibilidad de Android sin
cambios, más lo que TalkBack en Wear hace distinto — mira
[Accesibilidad en Android](/es/accessibility/android/).
