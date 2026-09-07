---
title: Accesibilidad en Apple
description: Dónde aterrizan las seis props de accesibilidad en UIKit, AppKit y SwiftUI, qué no tiene equivalente en cada una, y cómo se comprueba leyendo el árbol desde fuera de la app.
sidebar:
  order: 2
---

Seis props en todos los primitivos dicen qué se le cuenta a un lector de
pantalla sobre una vista:

```html
<an-view
  [accessibilityRole]="'button'"
  [accessibilityLabel]="'Reproducir'"
  [accessibilityHint]="'Empieza la pista desde el principio'"
  [accessibilityValue]="'sesenta por ciento'"
  [accessibilityState]="{ selected: true, disabled: true }"
  [accessible]="true"></an-view>
```

Se declaran una vez, en `NativeVisual`, y significan lo mismo en todas partes.
Lo que **no** es igual en ninguna es cómo se escriben: las tres APIs de
accesibilidad de Apple no son una API con tres juegos de nombres, y las
diferencias son lo bastante grandes como para que cada host resuelva esto a su
manera.

## Tres APIs, tres formas

**UIKit** — el rol es un bit de un `u64`. `accessibilityTraits` es una máscara
donde lo que una vista *es* (botón, cabecera, imagen), a lo que *se parece*
ahora mismo (deshabilitada, seleccionada) y lo que *hace* (reproduce sonido,
pasa páginas) comparten todos una palabra. El contrato separa el rol del estado,
así que el host tiene que volver a juntarlos, y tiene que escribir la máscara
entera cada vez: poner un bit significa conocer los otros sesenta y tres.

**AppKit** — el rol es una cadena, y los estados no van dentro.
`setAccessibilityRole:` toma exactamente un rol, y `disabled`, `selected` y
`expanded` son propiedades separadas del protocolo `NSAccessibility`. Nada que
reconstruir, y sitio para cosas que UIKit no sabe decir.

**SwiftUI**, en el reloj — ninguna de las dos, porque no hay ninguna vista viva
a la que llamarle un setter. El árbol se reconstruye a partir de cada foto, así
que la traducción ocurre en Rust mientras se construye la foto y el shell solo
engancha `.accessibilityLabel()`, `.accessibilityAddTraits()` y el resto a la
vista que está construyendo.

## El rol, plataforma por plataforma

| `accessibilityRole` | Rasgo UIKit | Rol AppKit | Rasgo SwiftUI |
|---|---|---|---|
| `button` | `.button` | `AXButton` | `.isButton` |
| `link` | `.link` | `AXLink` | `.isLink` |
| `header` | `.header` | `AXHeading` | `.isHeader` |
| `image` | `.image` | `AXImage` | `.isImage` |
| `text` | `.staticText` | `AXStaticText` | `.isStaticText` |
| `checkbox` | `.toggleButton` | `AXCheckBox` | `.isToggle` |
| `radio` | **ninguno** | `AXRadioButton` | **ninguno** |
| `switch` | `.toggleButton` | `AXCheckBox` + subrol `AXSwitch` | `.isToggle` |
| `slider` | `.adjustable` | `AXSlider` | **ninguno** |
| `search` | `.searchField` | `AXTextField` + subrol `AXSearchField` | `.isSearchField` |
| `summary` | `.summaryElement` | **ninguno** | `.isSummaryElement` |
| `none` | la máscara vaciada | `AXUnknown` | no se añade nada |

Tres celdas dicen **ninguno**, y ninguna de las tres se redondea al rasgo de al
lado:

- **`radio` no tiene rasgo en UIKit.** Y tampoco hay nada cerca. Un botón de
  radio en iOS se anuncia por su valor y por su posición en el grupo —«dos de
  cinco»— y eso es una estructura, no un rasgo. No se aplica nada y el host lo
  dice una vez, con el nombre de lo que se pidió.
- **`slider` no tiene rasgo en SwiftUI.** Ajustable en SwiftUI no es un rasgo
  sino una acción, `accessibilityAdjustableAction`. Enganchar una que no hiciera
  nada haría que el lector ofreciera un gesto que no lleva a ningún sitio.
- **`summary` no tiene rol en AppKit.** Es una idea de VoiceOver-en-iOS: el
  elemento que se lee solo al entrar en una pantalla, para resumirla. En un Mac
  no existe ese momento —no «entras» en una ventana— y no hay ni rol ni subrol
  parecido.

Todo lo de esa tabla es expresable en Android, que es una forma distinta de
plataforma y no una mejor: no tiene campo de rol en absoluto, así que la mayoría
de los roles entran como nombre de clase de widget y los tres que no tienen
widget entran como texto hablado desde una cadena traducida. Mira
[Accesibilidad en Android](/es/accessibility/android/).

Dos cosas más que esconde la tabla. **UIKit no distingue una casilla de un
interruptor**: tiene un solo rasgo, «un botón que se enciende y se apaga», y eso
describe a los dos — lo que los separa es el dibujo, y el dibujo no se lee.
SwiftUI hace lo mismo. Y **los subroles de AppKit no son decoración**: un campo
de búsqueda en AppKit *es* un `AXTextField`, y lo único que lo distingue de
cualquier otro campo es su subrol, así que poner solo el rol lo dejaría
indistinguible de lo que no es.

### `none` es un rol, y el rol que nombra es ningún rol

`accessibilityRole="none"` no es lo mismo que no escribir la prop. Quitar el
binding le devuelve a la vista lo que era — un botón del sistema vuelve a
anunciarse como botón. `none` le quita el rol, incluido el que llevaba el
control de debajo: UIKit vacía la máscara, AppKit pone `AXUnknown`. Es la misma
división que hace el host de Android, donde `none` es la clase
`android.view.View`, la que no significa nada.

El elemento se queda en el árbol de cualquier forma, así que un nombre encima
sigue leyéndose. Simplemente se anuncia como nada en particular.

## El estado, plataforma por plataforma

| `accessibilityState` | UIKit | AppKit | SwiftUI |
|---|---|---|---|
| `disabled` | rasgo `.notEnabled` | `setAccessibilityEnabled(false)` | **ninguno** |
| `selected` | rasgo `.selected` | `setAccessibilitySelected(true)` | `.isSelected` |
| `checked` | valor `"1"` / `"0"` | valor `0` / `1` / `2` | valor `"1"` / `"0"` |
| `checked: 'mixed'` | **ninguno** | valor `2` | **ninguno** |
| `expanded` | **ninguno** | `setAccessibilityExpanded(true)` | **ninguno** |
| `busy` | **ninguno** | **ninguno** | **ninguno** |

`checked` no es rasgo de nadie. Donde va es al valor, y entra con la convención
**propia** de la plataforma en lugar de con una palabra nuestra: un `UISwitch`
publica `"1"` o `"0"` y VoiceOver lo convierte en «activado» o «desactivado» en
el idioma que tenga el dispositivo. Una cadena escrita por este proyecto saldría
en inglés en un teléfono configurado en japonés. AppKit usa números para lo
mismo, y por eso el estado intermedio cabe ahí y en ningún otro sitio — una
casilla de macOS publica de verdad un `2`.

Ese valor solo se escribe encima de uno que nadie reclamó. Si la plantilla puso
`accessibilityValue`, gana el de la plantilla y `checked` no lo toca.

`busy` no tiene forma en ninguna plataforma de Apple. Lo más cercano en AppKit
es el rol `AXBusyIndicator`, que es *un spinner* —otra vista, no un estado de
esta— así que ponerlo convertiría un botón en un spinner. Android sí tiene dónde
ponerlo, en el texto hablado, y por eso la fila de arriba es la única donde las
tres plataformas de Apple están vacías y Android no.

## Lo que el sistema ya acertó

Un `UIButton`, un `UISwitch` y un `NSButton` llegan con su nombre y su rol ya
puestos por el sistema: la etiqueta de accesibilidad de un `NSButton` es su
título, un `NSSwitch` ya es un `AXCheckBox` con el subrol `AXSwitch`. Escribir
encima de eso sin mirar empeora las cosas, y el peor caso es silencioso: una
etiqueta vacía no es «sin etiqueta», es un nombre que sustituye al bueno, y el
control se queda mudo.

Así que la regla es que **solo se sobreescribe lo que la plantilla puso de
verdad**: una cadena vacía o un binding quitado eliminan nuestra etiqueta en
lugar de escribir una vacía.

En UIKit ahí se acaba la historia — quitar la etiqueta devuelve la propia del
control, y los rasgos que llevaba una vista se guardan y se restauran igual.

**En AppKit no**, y este es el filo más afilado de toda el área. Sobreescribir
*cualquier cosa* en un `NSView` —basta una etiqueta— hace que AppKit deje de
calcular la accesibilidad de esa vista por su cuenta y empiece a servir lo
sobreescrito. El rol, que nadie sobreescribió, vuelve entonces como `AXUnknown`:
un botón al que solo se le dio un nombre mejor deja de anunciarse como botón.

No se arregla guardando el rol y volviéndolo a poner, porque el rol no se puede
leer. Dentro del proceso, `NSButton.accessibilityRole()` responde `AXUnknown`
mientras que a un cliente asistivo de verdad se le dice `AXButton` — AppKit lo
calcula en la celda, bajo demanda, para el cliente. Así que lo que el host
vuelve a escribir no es un valor guardado sino el rol que el primitivo tiene de
verdad: un `an-button` monta un `NSButton` y un `NSButton` es un `AXButton`, que
es el mismo hecho que el inventario de la plataforma ya declara como nombre de
clase.

Cuando **sí** se da un rol, sustituye en lugar de sumar. Una plantilla que
escribe `accessibilityRole="link"` en un `an-button` está diciendo que esto se
lee como un enlace, no como «enlace, botón».

## Un rol o un nombre por sí solos no bastan

Ni `UIView` ni `NSView` son elementos de accesibilidad por defecto. Un `an-view`
pelado al que se le da `accessibilityRole="slider"` y nada más tiene el rol bien
puesto, no se publica nunca a un lector, y no produce ningún error en ninguna
parte. Lo mismo con una vista a la que solo se le da una etiqueta — que es el
uso más común de todo el contrato, y era el fallo más silencioso que tenía.

Así que en los dos hosts un rol que la plataforma pueda honrar convierte la
vista en elemento, y un nombre también. `accessible` manda sobre los dos, porque
es la prop que existe para decir exactamente esto:

| lo que dijo la plantilla | ¿es una parada? |
|---|---|
| `accessible="true"` | sí, y lo de dentro deja de ser paradas separadas |
| `accessible="false"` | no, y lo de dentro tampoco |
| un rol que la plataforma tenga, o una etiqueta | sí |
| nada | lo que la vista ya fuera |

`accessible="false"` tiene que llegar también a los hijos. Quitar el elemento
por su cuenta no esconde nada: UIKit necesita `accessibilityElementsHidden`, y
AppKit necesita la lista de hijos vaciada, o una caja decorativa con una
etiqueta dentro sigue siendo una parada.

## Cómo se comprueba esto

**Poner una propiedad no demuestra que un lector la pueda leer.** Todos los
fallos de arriba —el rol perdido, la vista que no es elemento, la etiqueta vacía
encima de una buena— pasan cualquier comprobación que relea la propiedad que
acaba de escribir, y ninguno aparece en un log ni en una captura. Todos ellos se
encontraron leyendo el árbol desde fuera de la app, y ninguno se habría
encontrado de otra manera.

```bash
./scripts/check-accessibility.sh            # en check-all.sh
./scripts/check-accessibility-simulator.sh  # necesita un simulador; se lanza a propósito
```

Las dos lanzan la app y leen su árbol desde un **proceso separado** —
`scripts/ax-dump.swift`, que nunca enlaza contra ella y no tiene más que un
pid— a través de `AXUIElementCopyAttributeValue`, la misma puerta que usan
VoiceOver y el Accessibility Inspector.

En **macOS** la app corre en la propia máquina. En **iOS** funciona porque el
Simulator puentea el árbol de la app invitada hacia la API de accesibilidad del
anfitrión, que es como el Accessibility Inspector inspecciona un simulador; el
recorredor le pregunta a `Simulator.app` en lugar de a la app. Lo que vuelve es
la máscara de UIKit ya traducida a atributos AX por la plataforma:

```text
AXGroup[iOSContentGroup]
  AXHeading label="Settings"
  AXButton label="Save the draft" help="saves it without leaving the screen"
  AXLink label="Open the website"
  AXCheckBox[AXSwitch] label="Select all"
  AXCheckBox[AXSwitch] label="Alerts" value="1"
  AXGenericElement label="Volume" value="35 %" enabled=false selected=true
  AXGenericElement label="An option"
  AXSlider label="A range"
  AXButton label="OK"
  AXButton label="Save the changes you made"
  AXGenericElement label="Stripped of its role"
```

Dos filas de ahí merecen leerse dos veces. `Select all` pidió
`checked: 'mixed'` y volvió **sin ningún valor**, mientras que `Alerts` pidió
`checked: true` y volvió con `"1"` — que es el estado intermedio sin sitio
adonde ir en UIKit, visible en lugar de afirmado. Y `An option` pidió
`role="radio"`: es una parada, porque tiene nombre, y no se publica como nada,
porque no existe tal rasgo que publicar.

La mitad de las aserciones son sobre lo que **no** debe estar ahí, porque una
comprobación que solo busca lo que espera no puede cazar el fallo contrario: la
fila decorativa no está, el icono y el texto de la fila agrupada no son dos
paradas propias de más, y no se publica nada como `AXRadioButton` en iOS, donde
no existe ese rasgo.

Las dos necesitan el permiso de Accesibilidad, que se concede a mano por app en
Ajustes del Sistema y no se puede conceder desde un script. Sin él lo dicen y se
saltan: una máquina sin la concesión no ha roto nada, y una comprobación que
fallara por ese motivo sería una comprobación que nadie se cree.

### Lo que no se verifica así

**tvOS y visionOS** montan el mismo host de UIKit que iOS, así que el mapeo es
el mismo código — pero sus simuladores no se recorrieron, así que eso es
inferencia, no evidencia.

**watchOS** no tiene ninguna ruta desde fuera. Lo que se comprueba ahí es lo que
se puede: que las seis props viajan con los nombres correctos, que el
vocabulario de roles del core es exactamente el que declara el contrato, que
cada nombre de rasgo que emite Rust existe en la tabla de Swift que lo convierte
en un `AccessibilityTraits`, y que el shell pasa el chequeo de tipos. Nada de
eso es lo mismo que saber que un lector lo ve.

**VoiceOver nunca estuvo en marcha.** Se puede encender en los defaults del
simulador, y la app corre entonces con normalidad — pero el proceso de VoiceOver
no arranca, no se dice nada y no aparece ningún cursor. Leer el árbol es lo más
cerca que llega esto, y conviene tener claro que «el árbol que vería un cliente
asistivo» y «un lector de pantalla dijo las palabras correctas» son dos
afirmaciones distintas.

## El ejemplo

`examples/a11y` es la pantalla que leen todas las comprobaciones de
accesibilidad — el volcado del dispositivo Android además de estas dos. Es una
sola pantalla y no una por plataforma a propósito: las filas que importan son
casi las mismas filas, y donde una plataforma difiere es mucho más útil ver la
diferencia en la misma línea que comparar dos ficheros.

No está pensada para mirarla. Cada fila es un caso sobre el que alguno de los
hosts tuvo que decidir algo, incluidos los que ninguna plataforma puede
expresar, para que «se dice en voz alta» sea algo que una comprobación pueda
leer en el log en lugar de algo que afirma un comentario.

```bash
cargo an macos examples/a11y     # en esta máquina
cargo an ios examples/a11y       # en el simulador
```
