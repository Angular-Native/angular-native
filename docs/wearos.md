# Wear OS

Angular en el reloj de Android: mismo núcleo, mismo layout, mismo bundle y
—esta vez sí— el mismo host.

```bash
cargo an wearos examples/hello-wear
./scripts/check-wearos.sh
```

## Por qué esto no se parece al reloj de Apple

[docs/watchos.md](watchos.md) cuenta que el reloj de Apple no tiene jerarquía
de `UIView` y que por eso hubo que escribir un host declarativo entero: el
árbol de Rust se refleja en un modelo que redibuja SwiftUI.

En Wear OS no pasa nada de eso. Un reloj con Wear OS es Android: `Activity`,
`Choreographer`, `android.view.View`, `ViewGroup.addView`, `setBackground`. El
host que ya existía —`crates/an-android` más las 4.500 líneas de
`shells/android/java`— vale tal cual.

Y vale literalmente: lo primero que se probó fue instalar el APK del teléfono,
sin tocar nada, en un emulador de Wear OS 5. Arrancó, evaluó el bundle y pintó:

```text
09-02 08:05:25.494 I angular-native: Angular is running in development mode.
09-02 08:05:25.522 I ActivityTaskManager: Displayed dev.angularnative/.MainActivity +643ms
```

O sea que la parte cara —QuickJS, el puente binario, taffy, las vistas— no
tuvo que hacerse. El trabajo del reloj es otro, y son cuatro cosas.

## 1. El APK del reloj no es el APK del teléfono

Aunque el de teléfono arranque. Lo que lo convierte en una app de reloj es una
línea del manifiesto:

```xml
<uses-feature android:name="android.hardware.type.watch" android:required="true" />
```

Sin ella, para el sistema y para la tienda esto es una app de teléfono que
casualmente se instaló en un reloj. Es el fallo que no se ve: se instala igual,
arranca igual y pinta igual. Solo se nota al publicarla.

Y como es un fichero entero el que cambia —`aapt2` no sabe de variantes ni de
condicionales—, el manifiesto del reloj es otro fichero,
[`shells/android/AndroidManifest.wear.xml`](../shells/android/AndroidManifest.wear.xml),
y `an wearos` elige cuál enlaza.

El tema también cambia, y por eso salió del manifiesto a
`res/values/styles.xml`: el del reloj necesita `windowSwipeToDismiss`. En Wear
OS no hay botón de atrás; se arrastra desde el borde izquierdo. Los temas de
Wear del sistema lo traen puesto, los de Material 3 —que son de teléfono— no, y
sin él la app se queda sin salida sin dar ningún error: sencillamente no pasa
nada al deslizar.

`Theme.DeviceDefault`, que es lo que recomienda Google para una app de Wear con
vistas, no se puede usar: `MainActivity` es una `AppCompatActivity` y AppCompat
se niega a arrancar sobre un tema que no descienda de `Theme.AppCompat`.

## 2. La pantalla es redonda

Es lo que más cambia y lo que menos código costó, porque ya había sitio donde
ponerlo.

En una esfera, las esquinas no existen. Lo que se coloque ahí no sale recortado
con un aviso: no se pinta, y nadie dice nada. Es exactamente la misma pregunta
que hace el notch de un iPhone —«¿hasta dónde puedo pintar?»— y `an-safe-area`
ya la contesta en las tres plataformas, así que el margen de la curva viaja por
ahí y no por un evento nuevo. Una plantilla no tiene por qué saber si el margen
que le apartan viene de una muesca o de un arco.

Lo que da el sistema y lo que hay que deducir, que no es lo mismo:

- **Lo da el sistema:** que la pantalla es redonda
  (`Configuration.isScreenRound()`), y el mentón de los relojes que lo tienen,
  que sí llega como `WindowInsets`.
- **Hay que deducirlo:** cuánto hay que apartarse. El lado del cuadrado
  inscrito en una circunferencia de diámetro `d` es `d/√2`, así que sobra
  `d·(1 − 1/√2)` repartido entre los dos lados: **0,146447 del diámetro por
  lado**. Es la misma cifra que usa `BoxInsetLayout` de androidx.wear, que es el
  contenedor que Google da para esto.

No se usa `BoxInsetLayout` directamente aunque exista, y no por no traerse la
dependencia: es un `ViewGroup` que coloca a sus hijos, y aquí el layout lo lleva
taffy. Serían dos motores de layout decidiendo lo mismo, que es justo lo que
[docs/watchos.md](watchos.md) evita en el host de SwiftUI.

En el emulador de 454 px a densidad 2 son 227 puntos de diámetro y 33,2 de
margen por lado: el cuadrado útil son 160×160.

## 3. La corona

La corona digital de Wear OS **no es un toque**. Llega como `ACTION_SCROLL`
desde `SOURCE_ROTARY_ENCODER`, por el camino de los eventos genéricos
—`onGenericMotionEvent`— y no por el de los táctiles. Por eso el host no la
veía: `AnScrollView` solo miraba `onTouchEvent`.

Tres cosas que no son evidentes:

- **El valor va en muescas de rueda, no en píxeles.** Lo convierte
  `ViewConfiguration.getScaledVerticalScrollFactor()`, el mismo factor que usa
  un ratón.
- **Los eventos van a la vista con el foco.** Una lista no lo pide sola: sin
  `setFocusableInTouchMode(true)` el sistema los manda a quien lo tenga
  —normalmente a nadie— y la corona no hace nada, sin error ninguno.
- **Eso solo se enciende en el reloj.** En un teléfono, una lista que reclama el
  foco se lo quita al campo de texto que hubiera debajo.

Desde la plantilla no se declara nada. `an-scroll-view` la escucha, y `(scroll)`
sale igual que si el dedo la hubiera arrastrado, con lo cual `an-virtual-list`
—que se apoya en el mismo scroll— también se recorre con la corona sin tocarla.

Lo que **no** hace hoy: alimentar un `an-slider` o un `an-stepper`. Eso pide un
evento propio en las primitivas, y las primitivas no se tocaron en esta tanda.

## 4. Lo que no va en un reloj

El resto de la barra de pestañas y el cromo de teléfono no se dejan salir mal.
Cuando el host detecta que está en un reloj —se lo pregunta al sistema con
`PackageManager.FEATURE_WATCH`, no al manifiesto, porque el APK del teléfono se
puede instalar en un reloj y entonces el manifiesto miente—, estas primitivas no
se montan: en su lugar queda una marca visible con su nombre y un error en el
log con el motivo. Es el mismo trato que le da el host de watchOS.

| Primitiva | Por qué no |
|---|---|
| `an-tab-bar` | Wear OS no tiene barra de pestañas: se navega deslizando y con la corona |
| `an-segmented-control` | no cabe a lo ancho de una esfera |
| `an-navigation-bar` | arriba va la hora del sistema, y el «atrás» es el deslizamiento desde el borde |
| `an-search-bar` | buscar en el reloj no es un campo dentro de la pantalla, sino la pantalla de entrada del sistema —dictado, garabateo o teclado— |
| `an-web-view` | Wear OS no lleva WebView: no hay ningún paquete que implemente `android.webkit` en el sistema |
| `an-date-picker` | el selector de la plataforma es un calendario de teléfono; en el reloj la fecha se elige a pantalla completa |
| `an-select` | un desplegable abre un menú anclado, y en una esfera no hay dónde anclarlo |

La marca conserva lo suyo: no acepta el `[color]` ni el `[title]` de la
primitiva que sustituye. Si los aceptara se disfrazaría de ella —es un
`TextView`, así que el rótulo y el color le entrarían igual— y parecería que
funciona.

Lo que **sí** va, y por qué no está en la lista: `an-switch`, `an-slider`,
`an-button`, `an-progress-bar`, `an-activity-indicator`, `an-icon`,
`an-stepper`, `an-alert` y `an-modal` son controles de Material que se dibujan
igual en cualquier pantalla; `an-text-input` abre el teclado del sistema, que en
el reloj es la pantalla de entrada de Wear; y `an-video-view` y `an-map-view`
son vistas de la plataforma que existen en Wear OS.

## Cómo se prueba sin reloj

No hay ningún AVD de Wear por defecto. Se crea así:

```bash
sdkmanager "system-images;android-34;android-wear;arm64-v8a"
avdmanager create avd -n an-wear \
  -k "system-images;android-34;android-wear;arm64-v8a" -d wearos_large_round
```

Y hay que tocarle dos cosas del `config.ini`, que `avdmanager` deja en «no»:

```ini
hw.lcd.circular=true    # sin esto isScreenRound() es false y no hay margen redondo
hw.rotaryInput=yes      # sin esto no hay corona que girar
```

La corona se mueve desde el terminal, que es como se hicieron las capturas:

```bash
adb -s emulator-5556 shell input rotaryencoder scroll --axis SCROLL,-3   # abajo
adb -s emulator-5556 shell input rotaryencoder scroll --axis SCROLL,3    # arriba
```

El emulador de Wear vuelve a la esfera a los diez segundos de no tocarlo, como
un reloj de verdad. Para dejarlo quieto mientras se mira:

```bash
adb -s emulator-5556 shell settings put system screen_off_timeout 1800000
adb -s emulator-5556 shell svc power stayon true
```

`an wearos` elige el aparato preguntándole su forma
—`ro.build.characteristics` lleva `watch` en cualquier imagen de Wear OS—, así
que con un teléfono y un reloj arrancados a la vez cada APK va al suyo. Antes,
`adb install` a secas se plantaba en cuanto había más de uno.

## Qué falta

- **La corona como fuente de valor, no solo de desplazamiento.** Alimentar un
  `an-slider` o un `an-stepper` pide una salida nueva en `packages/primitives`.
- **`an-text-input`.** Se monta y abre el teclado del sistema, pero no se ha
  comprobado que el flujo de entrada de Wear —dictado y garabateo— devuelva el
  texto por donde el host lo espera.
- **Modo ambiente.** Un reloj de verdad baja a una pantalla en blanco y negro a
  1 Hz cuando se baja la muñeca. Hoy la app simplemente se cierra, que es lo que
  hace Wear OS con una app que no lo declara. Es otra superficie del sistema, con
  su propio ciclo de vida.
- **Complicaciones y esferas.** No comparten nada con esto.
- **Relojes cuadrados.** Funcionan —el margen redondo solo se aplica si el
  sistema dice que la pantalla lo es—, pero no se han probado.
