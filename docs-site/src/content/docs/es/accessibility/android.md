---
title: Accesibilidad en Android
description: Las seis props de accesibilidad del contrato, y dónde aterriza cada una en AccessibilityNodeInfo.
sidebar:
  order: 3
---

Una plantilla escribe seis props y un lector de pantalla anuncia algo. Entre las
dos cosas hay una traducción, y en Android es más larga de lo que parece: solo
dos de las seis son propiedades de una vista. El resto no existen como campos.

```html
<an-view
  [accessibilityRole]="'button'"
  [accessibilityLabel]="'Guardar el borrador'"
  [accessibilityHint]="'lo guarda sin salir de la pantalla'"
  (press)="save()">
  <an-text>Guardar</an-text>
</an-view>
```

Eso es un `AnViewGroup` —un `ViewGroup` pelado— y TalkBack lo anuncia como
«Guardar el borrador, botón, toca dos veces para lo guarda sin salir de la
pantalla». Nada de eso es una propiedad que puedas poner en la vista.

## Las dos mitades

**Lo que sí es propiedad de la vista** es el nombre y si esto cuenta como un
elemento:

| Prop | Android |
|---|---|
| `accessibilityLabel` | `View.setContentDescription` |
| `accessible` | `View.setImportantForAccessibility` y `ViewCompat.setScreenReaderFocusable` |

**Todo lo demás** —rol, valor y estado— vive en el `AccessibilityNodeInfo` que
una vista rellena cada vez que un servicio de accesibilidad pregunta por ella.
La única entrada es interceptar ese momento, que es para lo que sirve un
`AccessibilityDelegate`:

```java
@Override
public void onInitializeAccessibilityNodeInfo(View host, AccessibilityNodeInfoCompat info) {
    super.onInitializeAccessibilityNodeInfo(host, info);   // lo que dice el widget
    decorate(host, info, state);                           // lo que dijo la plantilla
}
```

El delegate se instala en el primer nodo que recibe una prop que lo necesita, y
en ningún otro. Una app que no use nada de esto no paga nada.

## El rol

Android no tiene campo de rol. Lo que usa un lector de pantalla para decir
«botón» es el nombre de clase del nodo, que por defecto es el de la vista real.
Siete de los doce roles tienen un widget cuyo nombre Android ya sabe anunciar,
en el idioma del sistema y con la palabra que use esa versión de TalkBack; tres
no, y pasan por `setRoleDescription`, que es texto que un lector pronuncia tal
cual — así que sale de `res/values/strings.xml` y está traducido, no de una
constante en el código.

| `NativeRole` | `AccessibilityNodeInfo` |
|---|---|
| `button` | `setClassName("android.widget.Button")` |
| `image` | `setClassName("android.widget.ImageView")` |
| `text` | `setClassName("android.widget.TextView")` |
| `checkbox` | `setClassName("android.widget.CheckBox")` + `setCheckable(true)` |
| `radio` | `setClassName("android.widget.RadioButton")` + `setCheckable(true)` |
| `switch` | `setClassName("android.widget.Switch")` + `setCheckable(true)` |
| `slider` | `setClassName("android.widget.SeekBar")` |
| `header` | `setHeading(true)` — una bandera, no una clase; es por lo que TalkBack navega entre cabeceras |
| `link` | `setRoleDescription`, desde una cadena traducida |
| `search` | `setRoleDescription`, desde una cadena traducida |
| `summary` | `setRoleDescription`, desde una cadena traducida |
| `none` | `setClassName("android.view.View")` — la clase que no significa nada, y la única forma de quitarle el rol al widget de debajo |

Un rol que el host no conoce es un error en el log, no un no-op silencioso.
Debería ser inalcanzable —el compilador de Angular rechaza cualquier cosa fuera
de `NativeRole`— así que llegar ahí significa que el contrato y el host se han
separado, y `check-a11y.sh` es lo que evita que eso pase.

## El estado

| `NativeAccessibilityState` | `AccessibilityNodeInfo` |
|---|---|
| `disabled` | `setEnabled(!disabled)` |
| `selected` | `setSelected` |
| `checked: true \| false` | `setCheckable(true)` + `setChecked` |
| `checked: 'mixed'` | `setCheckable(true)` + `setChecked(false)` + «parcialmente marcado» en `setStateDescription` |
| `expanded` | `addAction(ACTION_EXPAND)` o `ACTION_COLLAPSE` |
| `busy` | «ocupado» en `setStateDescription` |

Tres cosas conviene saberlas.

**`disabled` solo toca el nodo.** Apagar la vista de verdad es lo que hace
`[enabled]`, y eso además deja de responder al tacto. Si `accessibilityState`
hiciera lo mismo, describir un estado cambiaría el comportamiento sin que nadie
lo hubiera pedido.

**`mixed` no tiene dónde ir.** `AccessibilityNodeInfo.setChecked` es un
booleano. El tercer estado es un nodo marcable que no está marcado, más un
`stateDescription` que lo dice con palabras — lo mismo que hace Jetpack Compose
con `ToggleableState.Indeterminate`. Ni se descarta, ni se falsea como `true`.

**`expanded` es una acción, no un campo.** En Android, expandir algo es
`ACTION_EXPAND`; un lector anuncia «toca dos veces para expandir». Así que solo
se pone en un nodo que responda al tacto. En uno que no, el host registra un
error y no pone nada, porque anunciar una acción que no puede ocurrir es peor
que no anunciar nada.

## El valor

`accessibilityValue` va a `setStateDescription`, la ranura que abrió Android 11
para exactamente esto: cuánto vale un control ahora mismo, anunciado después del
nombre y del rol.

Tres cosas pueden querer esa única ranura —el valor, el «parcialmente marcado»
de `mixed` y `busy`— y se unen con comas en lugar de ganar una. La plantilla
pidió las tres; descartar dos sería perderlas en silencio.

## La pista

Android tiene una sola ranura de pista y un lector la pronuncia en los campos de
texto, así que siempre va a `setHintText`. En algo que se pulsa, lo que un
lector lee de verdad es la etiqueta de la acción de clic, así que la pista va
también ahí:

```java
info.setHintText(hint);
if (host.isClickable()) {
    info.addAction(new AccessibilityActionCompat(ACTION_CLICK, hint));
}
```

Eso es lo que convierte «toca dos veces para activar» en «toca dos veces para lo
guarda sin salir de la pantalla».

## `accessible`, y el orden que era un fallo

`accessible: true` es `IMPORTANT_FOR_ACCESSIBILITY_YES` más
`setScreenReaderFocusable(true)`: una parada en lugar de tres para una fila
hecha de un icono, un título y un subtítulo. `accessible: false` es
`IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS` — un `NO` a secas escondería
la vista y dejaría a sus hijos en el árbol, colgando del abuelo, y la decoración
tiene que irse entera.

Lo que no es obvio es que `setContentDescription` no es solo un setter: en una
vista que sigue en `AUTO` la promociona a `YES`. Sin esa promoción, un `an-view`
con nombre y nada más —sin tacto, sin rol, sin estado— se queda en un contenedor
que el sistema encuentra poco interesante, y el nombre no lo lee nadie: el nodo
ni siquiera aparece en el árbol que recorre un lector de pantalla.

Así que la importancia se escribe **primero** y el nombre **después**. Escrito
al revés, la promoción se deshace y la etiqueta se queda muda. Esto no se cazó
leyendo el código; se cazó con el volcado, y por eso existe el volcado.

## Lo que no se sobreescribe

Un `an-button` es un `MaterialButton` y un `an-switch` es un `MaterialSwitch`.
Los dos ya responden correctamente: el botón se anuncia como botón, y el
interruptor dice si está encendido. Escribir nuestro rol y nuestro estado encima
por defecto sustituiría algo que está bien por algo que supusimos.

Así que cada campo del estado del nodo es nulo hasta que la plantilla lo pone,
el delegate llama primero al de debajo, y solo se escriben los campos que
llegaron. Un control sin ninguna prop de accesibilidad llega al árbol de
accesibilidad exactamente como lo dejó Material, y `check-a11y-device.sh` lo
afirma: busca en el volcado un `Switch` con la descripción de contenido vacía
que se declara encendido.

`testID` es la única colisión. En iOS es `accessibilityIdentifier`, un campo
aparte; en Android no hay un segundo sitio — el `resource-id` que sería su
equivalente solo acepta enteros de la `R` de la app. Así que los dos acaban en
`contentDescription`, y la regla es que gana la etiqueta: `testID` rellena el
nombre solo cuando no hay `accessibilityLabel`. Dejar ganar al que llegara el
último sería un orden que la plantilla no controla.

## Wear OS

Wear OS es Android, así que todo esto aplica sin añadir nada: la misma
`android.view.View`, el mismo `AccessibilityNodeInfo`, el mismo delegate. El
lector de pantalla también es TalkBack. Nada de esta página depende de la forma
del dispositivo.

Eso se comprobó y no se supuso: el mismo APK compilado con `an wearos`,
instalado en un emulador de Wear OS 5, vuelca las mismas clases, las mismas
descripciones de contenido y los mismos checkable/checked/selected/enabled para
cada fila que quepa en una pantalla redonda de 227 puntos.

Aun así hay tres cosas que merece la pena decir. `check-a11y-device.sh` se niega
a correr en un reloj, y dice por qué: este ejemplo maqueta más filas de las que
caben en una esfera, lo que queda fuera de pantalla tampoco está en el árbol de
accesibilidad, y la comprobación fallaría por el tamaño de la pantalla mientras
se leería como si las etiquetas no hubieran llegado. El volcador de la imagen
Wear —Android 14— no escribe el atributo `hint` en absoluto, donde el de Android
16 sí; la comprobación lo nota y dice que la pista se quedó sin comprobar en
lugar de suspender por una herramienta que nunca informó de ella. Y los
primitivos que Wear OS no monta —`an-tab-bar`, `an-select` y los otros cinco de
[Wear OS](/es/platforms/wearos/)— dejan un marcador visible en su lugar, y ese
marcador se guarda lo suyo: no toma ninguna prop del primitivo al que sustituye,
accesibilidad incluida, precisamente para que no pueda disfrazarse del control
que no está.

La corona es lo único que podría haber necesitado algo propio y no lo necesita:
es un codificador rotatorio, no un toque, así que no se acerca nunca a las
acciones de accesibilidad.

## Verlo, no afirmarlo

Una prop de accesibilidad que no llega a nadie no revienta, no registra nada y
tiene exactamente el mismo aspecto que una que funciona. Solo se puede ver con
un lector de pantalla encendido, o con un volcado — y Android tiene volcado:

```bash
adb shell uiautomator dump --compressed /sdcard/tree.xml
adb shell cat /sdcard/tree.xml
```

Eso serializa el árbol de `AccessibilityNodeInfo` que construyó la plataforma,
que es el mismo árbol que recorre TalkBack. No es nuestra palabra.

Las dos banderas importan y no son el mismo árbol:

- **sin bandera** incluye vistas que el sistema no considera importantes para la
  accesibilidad, porque UiAutomation las pide para que un test de UI pueda
  llegar a cualquier cosa;
- **`--compressed`** no. Ese es el árbol del lector de pantalla, y es donde
  `accessible: false` tiene que haber quitado una fila junto con su contenido.

```bash
./scripts/check-a11y.sh                       # solo texto, corre en check-all
./scripts/check-a11y-device.sh emulator-5554  # compila, instala, vuelca, compara
```

`check-a11y-device.sh` compila `examples/a11y`, lo instala, espera a la
pantalla, vuelca los dos árboles y los compara con lo que pidió la plantilla.
Las expectativas no están escritas en la comprobación: se leen de la plantilla
del ejemplo y de la tabla de roles del propio Java del host, así que la
comprobación no puede decir que la plantilla pidió algo que no pidió.

Está fuera de `check-all.sh` porque necesita un dispositivo y unos dos minutos.
La mitad que no cuesta nada —que el contrato, el host, las cadenas y esta página
sigan diciendo lo mismo— sí está enganchada ahí.

## Lo que el volcado no puede enseñar

`uiautomator dump` enseña `content-desc`, `hint`, `class`, `checkable`,
`checked`, `selected`, `enabled` y `clickable`, y si un nodo está siquiera en el
árbol. Eso cubre la etiqueta, la pista, los siete roles que mapean a una clase,
`none`, todo `checked`/`selected`/`disabled`, y las dos mitades de `accessible`.

No enseña `roleDescription`, `heading`, `stateDescription` ni las etiquetas de
las acciones. Así que `link`, `search`, `summary`, `header`,
`accessibilityValue`, `busy`, el «parcialmente marcado» de `mixed` y la pista de
un nodo pulsable se ponen y se comprueban unitariamente en Java, pero no los
demuestra el volcado. Demostrarlos necesita un lector de pantalla en marcha
leyendo en voz alta, que es otra clase de comprobación y no está escrita.
