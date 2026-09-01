# El envoltorio configurable

Cada primitiva envuelve un control nativo de verdad. La pregunta que contesta
este documento es cuánto de ese control se puede tocar desde una plantilla
Angular, y qué queda todavía fuera del alcance.

## Las tres reglas

1. **Lo que existe en las dos plataformas se llama igual** y es una entrada
   normal: `[variant]`, `[placeholder]`, `[enabled]`. El nombre lo elige la
   API, no la plataforma que lo tenga más bonito.
2. **Lo que solo existe en una va en su objeto**: `[ios]="{…}"` y
   `[android]="{…}"`. Nunca se inventa un nombre común para algo que solo
   tiene una plataforma: obligaría a la otra a imitarlo, y una imitación se
   nota.
3. **Nada se pierde en silencio.** Una clave que el host no reconozca avisa
   una vez por consola, igual que hace `warnUnknownStyle()` con los estilos.
   Ese fallo —la prop que viaja, nadie mira y no da error— ya costó cuatro
   tardes con los estilos.

Las dos primeras las comprueba el compilador de plantillas: las props son
entradas declaradas y los objetos por plataforma son tipos cerrados, así que
`[ios]="{ subtitulo: 'x' }"` no compila. La tercera es para lo que el
compilador no ve: un objeto construido a mano, un `spread`, un `any` que se
coló.

Que la prop llegue de verdad al otro extremo lo comprueba
`scripts/check-wrapper.sh`, que compara la lista de props de las directivas con
lo que miran `crates/an-ios/src/host.rs` y `AnHost.java`. Que además haga algo
lo comprueban los scripts sobre el volcado headless, que desde ahora imprime
las props de cada nodo.

## Cómo viaja una prop

Las props son entradas de señal —`input()`—, no `@Input() set`. Una entrada de
señal no tiene un momento en el que "se asigna": se lee, y quien la lee decide
cuándo. Aquí la lee un efecto por directiva, no uno por prop: un `<an-text>`
declara once props y casi ninguna plantilla usa más de tres, así que un efecto
por prop haría que cada `<an-text>` de una lista de cinco mil filas cargase con
once nodos reactivos que nadie va a despertar. Leer once señales cuando cambia
una sale más barato.

Del efecto solo sale lo que cambió, y en la primera pasada se callan además
los nulos: es lo que vale una entrada que nadie ha puesto, y mandarlos sería
pedirle al host que borre algo que nunca escribió.

Un efecto de más sale gratis en un sitio: `<an-icon>` sin `[size]` ya no se queda
sin tamaño. Un `set` que nadie enlaza no corre nunca —y por eso el constructor
tenía que llamarlo a mano—, mientras que una señal se lee aunque nadie la
escriba.

## Cómo viaja una prop de plataforma

`[ios]="{ subtitle: 'x' }"` no manda un objeto: la directiva lo descompone y
manda `ios:subtitle`. El prefijo hace dos cosas. La primera, que el host de
Android pueda descartarla sin saber qué es. La segunda, que el nombre siga
siendo greppable: `check-wrapper.sh` exige que `"ios:subtitle"` aparezca en el
host de iOS y **no** aparezca en el de Android — si aparece en los dos es que
era una prop común y no debería llevar prefijo.

---

## Inventario

Leyenda: **hoy** = lo que ya existía; **nuevo** = lo que añade este trabajo;
**fuera** = lo que se decidió no hacer, con el motivo.

### an-button — `UIButton` · `MaterialButton`

| Prop | iOS | Android | Estado |
|---|---|---|---|
| `title` | `setTitle:forState:` | `setText` | hoy |
| `color` | `tintColor` | tinte según variante | hoy |
| `variant` `text\|filled\|tonal\|outlined` | `UIButtonConfiguration` | estilo de Material | `outlined` nuevo |
| `enabled` | `UIControl.isEnabled` | `setEnabled` | nuevo |
| `icon` | `configuration.image` (SF Symbol) | `setIcon` (Material Symbol) | nuevo |
| `iconPosition` `leading\|trailing` | `imagePlacement` | `setIconGravity` | nuevo |
| `fontSize` | `titleLabel.font` | `setTextSize` | nuevo |
| `fontWeight` | `titleLabel.font` | `setTypeface` | nuevo |
| `[ios].subtitle` | `configuration.subtitle` | — | nuevo |
| `[android].rippleColor` | — | `setRippleColor` | nuevo |
| `[android].allCaps` | — | `setAllCaps` | nuevo |

**Fuera:** `elevated` como variante — Material la tiene y UIKit no tiene nada
equivalente, así que sería una variante que solo hace algo en media
plataforma; quien la quiera la pide por `[android]`. `contentInsets` de
`UIButtonConfiguration` — el hueco interior de un botón lo decide el sistema y
tocarlo es justo lo que hace que un botón deje de parecer de la plataforma.
`cornerRadius` — no hace falta: `[borderRadius]` es una prop de cualquier
vista y el botón es una vista, así que ya redondea.

**Dos cosas que el botón de iOS no da, y no es por no intentarlo:**

*El rótulo de la variante `filled` no se dibuja.* El botón sale entero, con su
relleno, su icono y su subtítulo, y sin texto. Se ha probado con el rótulo
puesto en la configuración y con `setTitle:forState:`; con
`filledButtonConfiguration` y con `borderedProminentButtonConfiguration`; con
el color de fondo y el del rótulo fijados a mano y dejados al sistema; con un
color claro y con uno oscuro, por si fuera contraste; con icono y sin él; con
subtítulo y sin él; y sin mezclar ninguna llamada de las de siempre con la
configuración. El rótulo llega al host —se ve en el volcado headless— y las
otras tres variantes lo dibujan sin problema con el mismo código. Viene de
antes de este trabajo: la variante ya estaba y ya salía así. Queda por
resolver.

*`[fontSize]` y `[fontWeight]` mandan de verdad solo en la variante `text`.*
En las demás hay configuración, y con configuración UIKit resuelve la
tipografía por su cuenta: se le pide al rótulo la fuente igualmente, pero él
decide. Pedírselo de verdad es un `titleTextAttributesTransformer`, que es un
bloque que devuelve el diccionario de atributos, y eso no cabía en esta tanda.
En Android no hay caso: un `MaterialButton` es un `TextView` y la letra se le
pone y punto.

**Ojo con el subtítulo:** un botón con dos líneas no cabe en el alto natural
de un botón. El tamaño de cada control se le pregunta a la plataforma una vez
al arrancar, con un control de muestra —crear un `UIButton` exige el hilo
principal y el medidor vive en el del motor—, así que ese botón de muestra no
puede saber que este va a llevar subtítulo. Al ponerlo hay que darle alto en
la plantilla; si no, el rótulo se recorta y el subtítulo se queda solo.

### an-text-input — `UITextField` · `EditText`

| Prop | iOS | Android | Estado |
|---|---|---|---|
| `value`, `placeholder`, `editable` | | | hoy |
| `secureTextEntry` | `isSecureTextEntry` | `InputType` de contraseña | hoy en iOS, **fuga cerrada** en Android |
| `color`, `fontSize` | `textColor`, `font` | `setTextColor`, `setTextSize` | hoy (en iOS `fontSize` **no llegaba**: `apply_font` solo miraba `UILabel`) |
| `keyboardType` | `keyboardType` | `InputType` | nuevo |
| `returnKeyType` | `returnKeyType` | `imeOptions` | nuevo |
| `autoCapitalize` | `autocapitalizationType` | banderas de `InputType` | nuevo |
| `autoCorrect` | `autocorrectionType` | `NO_SUGGESTIONS` | nuevo |
| `placeholderColor` | `attributedPlaceholder` | `setHintTextColor` | nuevo |
| `textAlign` | `textAlignment` | `gravity` | nuevo |
| `fontWeight`, `fontFamily` | `font` | `setTypeface` | nuevo |
| `[ios].clearButtonMode` | `clearButtonMode` | — | nuevo |
| `[ios].borderStyle` | `borderStyle` | — | nuevo |
| `[android].selectAllOnFocus` | — | `setSelectAllOnFocus` | nuevo |
| `[android].cursorVisible` | — | `setCursorVisible` | nuevo |

**Fuera:** `maxLength`. Android lo hace con un `InputFilter` de una línea; iOS
no lo tiene y hay que interceptar
`textField:shouldChangeCharactersInRange:replacementString:`, lo que obliga a
meter un delegado propio en un control que hoy solo lleva acciones. Se puede
hacer, pero no cabe en esta tanda y a medias no vale: un `maxLength` que solo
funciona en Android es peor que no tenerlo.

### an-text — `UILabel` · `TextView`

| Prop | iOS | Android | Estado |
|---|---|---|---|
| `color`, `fontSize`, `fontWeight`, `fontStyle`, `fontFamily` | | | hoy (`fontStyle` y `fontFamily`, **fuga cerrada** en Android) |
| `textAlign`, `numberOfLines` | | | hoy |
| `lineHeight` | `NSParagraphStyle` | `setLineSpacing` | **fuga cerrada**: lo medía el núcleo y no lo dibujaba nadie |
| `letterSpacing` | `NSAttributedString` (`kern`) | `setLetterSpacing` | **fuga cerrada**: igual |
| `textDecoration` `none\|underline\|lineThrough` | atributos del texto | banderas de `Paint` | nuevo |
| `[android].selectable` | — | `setTextIsSelectable` | nuevo |

**Fuera:** `adjustsFontSizeToFitWidth`. `UILabel` lo trae y `TextView` tiene el
autodimensionado desde API 26, así que sería una prop común legítima; lo que la
deja fuera es que el núcleo mide el texto por su cuenta para el layout y no
sabe encoger, así que la caja seguiría siendo la del tamaño grande. Hacerlo
bien es tocar la medición, no el host.

### an-switch — `UISwitch` · `MaterialSwitch`

| Prop | iOS | Android | Estado |
|---|---|---|---|
| `on`, `color` | `isOn`, `onTintColor` | `isChecked`, tinte de la vía | hoy |
| `enabled` | | | nuevo |
| `thumbColor` | `thumbTintColor` | `setThumbTintList` | nuevo |
| `[android].trackColor` | — | vía apagada | nuevo |

**Fuera:** el color de la vía apagada en iOS. `UISwitch` no lo expone; lo que
circula por ahí es ponerle `backgroundColor` y un `cornerRadius` de 16 a un
control del sistema para que se le vea el fondo por detrás, que se rompe en
cuanto Apple cambia el alto del control. Eso es exactamente el hack feo que no
se hace: en iOS se queda con el color del sistema.

### an-slider — `UISlider` · `Material Slider`

| Prop | iOS | Android | Estado |
|---|---|---|---|
| `value`, `minimumValue`, `maximumValue`, `color` | | | hoy |
| `enabled` | | | nuevo |
| `minimumTrackColor` | `minimumTrackTintColor` | `trackActiveTintList` | nuevo |
| `maximumTrackColor` | `maximumTrackTintColor` | `trackInactiveTintList` | nuevo |
| `thumbColor` | `thumbTintColor` | `thumbTintList` | nuevo |
| `[ios].continuous` | `isContinuous` | — | nuevo |
| `[android].stepSize` | — | `setStepSize` | nuevo |

**Fuera:** `stepSize` como prop común. `UISlider` es continuo y no tiene
pasos; redondear el valor en el host se puede, pero entonces el dedo va por un
sitio y el valor por otro, y el control deja de dar la respuesta táctil que da
el de Android, que sí se engancha a los pasos. Prometer «pasos» y dar dos
comportamientos distintos es peor que decir que solo Android los tiene.

### an-scroll-view — `UIScrollView` · `AnScrollView`

| Prop | iOS | Android | Estado |
|---|---|---|---|
| `showsScrollIndicator` | `showsVertical…` | `setVerticalScrollBarEnabled` | hoy en iOS, **fuga cerrada** en Android |
| `refreshing` | `UIRefreshControl` | arco propio | hoy en iOS, **fuga cerrada** en Android |
| `bounces` | `bounces` | `overScrollMode` | hoy en iOS, **fuga cerrada** en Android |
| `scrollEnabled` | `isScrollEnabled` | se traga el gesto | nuevo |
| `[ios].pagingEnabled` | `isPagingEnabled` | — | nuevo |
| `[ios].keyboardDismissMode` | `keyboardDismissMode` | — | nuevo |

**Fuera:** desplazamiento horizontal. No es una prop del host: el núcleo
calcula el `contentSize` suponiendo que se desborda hacia abajo, y darle la
vuelta es trabajo de `an-core`, no del envoltorio.

### an-tab-bar — `UITabBarController` · `AnTabBar`

| Prop | iOS | Android | Estado |
|---|---|---|---|
| `items`, `icons`, `selectedIndex`, `color` | | | hoy |
| `unselectedColor` | `unselectedItemTintColor` | color inactivo | nuevo |
| `[ios].translucent` | `isTranslucent` | — | nuevo |

**Fuera:** `badges`. En iOS es `UITabBarItem.badgeValue` y sale gratis; en
Android la barra es nuestra —la plataforma no trae ninguna— así que habría que
dibujar el globo a mano, y un globo dibujado a mano al lado de uno del sistema
no se parecen. Queda apuntado como trabajo de `AnTabBar`, no del envoltorio.

### an-select — `UIButton` + `UIMenu` · `Spinner`

| Prop | iOS | Android | Estado |
|---|---|---|---|
| `items`, `selectedIndex` | | | hoy |
| `enabled` | | | nuevo |

**Fuera:** `mode` (`dropdown` o `dialog`). El `Spinner` de Android lo decide en
el constructor y no se puede cambiar después, y en iOS no hay las dos formas:
un `UIMenu` es siempre un menú. Cambiarlo obligaría a destruir y rehacer la
vista al vuelo, que es justo lo que el árbol evita.

### Los demás controles

`an-segmented-control`, `an-stepper`, `an-search-bar` y `an-date-picker`
reciben `enabled`, igual que el botón, el interruptor, el deslizador y el
desplegable: los ocho heredan de `NativeControl`. `an-activity-indicator` y
`an-progress-bar` no lo reciben porque no se tocan, y los campos de texto
tampoco porque ya tienen `editable`, que es la misma idea con el nombre que usa
un campo.

`an-navigation-bar`, `an-image`, `an-textarea`, `an-web-view`, `an-map-view` y `an-video-view` se
quedan con las props que ya tenían. Cada uno da para su propia tanda —el
editor comparte casi todas las del campo de una línea, la imagen tiene el
recorte y la carga diferida, el navegador tiene JavaScript, cookies y zoom— y
ninguno se ha tocado aquí para no dejarlos a medias.

---

## Fugas encontradas por el camino

El inventario no era un ejercicio de estilo: la comparación entre lo que
declaran las directivas y lo que miran los hosts sacó ocho props que ya
existían en la API pública y no llegaban a ninguna parte.

| Prop | Dónde se perdía | Qué se veía |
|---|---|---|
| `secureTextEntry` | Android | una contraseña escrita en claro |
| `showsScrollIndicator` | Android | la barra siempre puesta |
| `refreshing` | Android | la ruedecilla no paraba nunca |
| `bounces` | Android | nada, el rebote no se podía quitar |
| `fontFamily`, `fontStyle` | Android | la tipografía por defecto |
| `lineHeight`, `letterSpacing` | los dos | el núcleo **medía** con ellas y el host dibujaba sin ellas: el texto se salía de su caja |

Las dos últimas son el caso peor: no es que la prop no hiciera nada, es que
hacía la mitad. El layout reservaba el sitio de un texto con más interlineado
del que luego se dibujaba.

`intrinsicWidth` e `intrinsicHeight` salen en la comparación y no son una fuga:
las consume el layout para reservar el hueco de una imagen que todavía no ha
cargado, y no tienen nada que decirle a ningún host. Están declaradas como tal
en `check-wrapper.sh`.
