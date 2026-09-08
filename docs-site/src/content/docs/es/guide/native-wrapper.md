---
title: Props y el envoltorio nativo
description: Cuánto del control real alcanza una plantilla, cómo viaja una prop, y qué se dejó fuera de cada una a propósito.
sidebar:
  order: 5
---

Cada primitivo envuelve un control nativo de verdad. La pregunta que responde
esta página es cuánto de ese control puede tocar una plantilla, y qué sigue
fuera de alcance.

## Las tres reglas

1. **Lo que existe en las dos plataformas se llama igual** y es un input
   normal: `[variant]`, `[placeholder]`, `[enabled]`. El nombre sale de la API,
   no de la plataforma que lo escriba más bonito.
2. **Lo que existe en una sola va en el objeto de esa**: `[ios]="{…}"` y
   `[android]="{…}"`. Nunca se inventa un nombre común para algo que solo tiene
   una plataforma — eso obligaría a la otra a imitarlo, y una imitación se nota.
3. **Nada se pierde en silencio.** Una clave que el host no reconoce avisa una
   vez en la consola, exactamente igual que un estilo desconocido.

Las dos primeras las comprueba el compilador de plantillas: las props son inputs
declarados y los objetos por plataforma son tipos cerrados, así que
`[ios]="{ subtitel: 'x' }"` no compila. La tercera es para lo que el compilador
no puede ver — un objeto ensamblado en tiempo de ejecución, un spread, un `any`
que se coló.

Que una prop llegue de verdad al otro extremo lo comprueba
`scripts/check-wrapper.sh`, que compara la lista de props de las directivas con
lo que leen de verdad los cuatro hosts: el crate de iOS, el shell de Android, el
crate de macOS y el de watchOS. Esos cuatro son todos los que hay — `an-ios`
sirve también a tvOS y visionOS, y `an-android` a Wear OS.

## Cómo viaja una prop

Las props son **inputs de señal** —`input()`— y no `@Input() set`. Un input de
señal no tiene un momento en el que «se asigna»: se lee, y quien lo lee decide
cuándo.

Aquí lo lee un efecto por grupo de props, no un efecto por prop. Un `<an-text>`
declara diez propias y casi ninguna plantilla usa más de tres, así que un efecto
por prop dejaría a cada `<an-text>` de una lista de cinco mil filas cargando con
diez nodos reactivos que nadie va a despertar. Leer diez señales cuando cambia
una sale más barato. En la práctica un primitivo instala un efecto por clase de
su cadena de herencia que declare props, más uno por objeto de plataforma — un
`<an-view>` instala uno, un `<an-text>` tres, un `<an-button>` cinco.

Del efecto sale solo lo que cambió, y en la primera pasada los nulos también se
callan: eso es lo que vale un input que nadie puso, y enviarlo sería pedirle al
host que borre algo que nunca escribió.

Hay un sitio donde un efecto de más se gana el sueldo: un `<an-icon>` sin
`[size]` ya no se queda sin tamaño. Un `set` que nadie enlaza no corre nunca —y
por eso antes el constructor tenía que llamarlo a mano— mientras que una señal
se lee aunque nadie escriba.

### Nombres que cambian en el cable

Cuatro inputs no viajan bajo su propio nombre, y conviene saberlo cuando estás
leyendo un log o un host:

| Input | Prop en el cable | Por qué |
|---|---|---|
| `an-stepper` `[step]` | `stepValue` | |
| `an-icon` `[size]`, `[weight]` | `iconSize`, `iconWeight` | `size` y `weight` son demasiado genéricos para reclamarlos. |
| `(rotation)` | el evento `rotate` | `[rotate]` ya es el input de la transformación. |

`[items]`, `[icons]`, `[buttons]` y `[accessibilityState]` viajan como cadenas
JSON, porque el tipo de valor del cable no tiene array ni objeto. Un
`accessibilityState` nulo se queda nulo en lugar de convertirse en `"{}"`.

## Cómo viaja una prop de plataforma

`[ios]="{ subtitle: 'x' }"` no envía un objeto: la directiva lo desmonta y envía
`ios:subtitle`. El prefijo hace dos cosas. Primero, el host de Android puede
descartarlo sin saber qué es. Segundo, el nombre sigue siendo greppeable: la
comprobación exige que `"ios:subtitle"` aparezca en el host de iOS y **no** en
el de Android — aparecer en los dos significa que era una prop común disfrazada
con un prefijo.

Dos detalles:

- **Una clave que desaparece se des-pone.** La directiva recuerda qué claves
  envió la vez anterior, y una que ya no está en el objeto se reenvía como nula,
  para que el control vuelva a su valor de fábrica en lugar de quedarse con lo
  último que le diste.
- **Una clave desconocida avisa una vez por clave**, igual que un estilo
  desconocido.

Los tipos por plataforma son alias de `type` y no interfaces, deliberadamente:
una interfaz no tiene firma de índice, así que no se podría pasar al código que
desmonta el objeto.

## Lo que ya tiene cada primitivo

Veinticinco inputs y diecisiete outputs viven en la base que extienden los
veinticinco primitivos — fondo, borde, opacidad, las transformaciones,
`animate`, `cursor`, `testID`, las seis props de accesibilidad y todos los
gestos. Están listados en [Componentes](/es/reference/components/).

Ocho primitivos extienden una segunda base que añade exactamente un input,
`[enabled]`: `an-button`, `an-switch`, `an-slider`, `an-segmented-control`,
`an-stepper`, `an-search-bar`, `an-select` y `an-date-picker`.
`an-activity-indicator` y `an-progress-bar` no lo tienen porque no se tocan, y
los campos de texto tampoco porque ya tienen `[editable]`, que es la misma idea
con el nombre que usa un campo.

Todo lo de abajo es lo que cada primitivo añade **encima** de eso.

## El inventario

### `an-button` — `UIButton` · `MaterialButton` · `NSButton`

| Prop | iOS | Android |
|---|---|---|
| `title` | `setTitle:forState:` | `setText` |
| `color` | `tintColor` | tinte por variante |
| `variant` `text\|filled\|tonal\|outlined` | `UIButtonConfiguration` | estilo de Material |
| `icon` | `configuration.image` (SF Symbol) | `setIcon` (Material Symbol) |
| `iconPosition` `leading\|trailing` | `imagePlacement` | `setIconGravity` |
| `fontSize`, `fontWeight` | `titleTextAttributesTransformer` | `setTextSize`, `setTypeface` |
| `[ios].subtitle` | `configuration.subtitle` | — |
| `[android].rippleColor` | — | `setRippleColor` |
| `[android].allCaps` | — | `setAllCaps` |

Un toque llega por el `(press)` heredado: el botón no tiene salida propia.

**Se dejó fuera:** `elevated` como variante — Material la tiene y UIKit no tiene
nada equivalente, así que sería una variante que hace algo en media plataforma;
pídela por `[android]`. `contentInsets` — el espacio dentro de un botón lo
decide el sistema, y tocarlo es exactamente lo que hace que un botón deje de
parecer el de la plataforma. `cornerRadius` — innecesario: `[borderRadius]` es
una prop de cualquier vista y un botón es una vista.

:::caution[Un subtítulo necesita una altura]
Un botón de dos líneas no cabe en la altura natural de un botón. El tamaño de
cada control se le pregunta a la plataforma una vez al arrancar, con un control
de muestra —crear un `UIButton` necesita el hilo principal y el medidor vive en
el del motor— así que esa muestra no puede saber que este va a llevar
subtítulo. Dale una altura en la plantilla, o el título se recorta y el
subtítulo se queda solo.
:::

### `an-text-input` — `UITextField` · `EditText` · `NSTextField`

| Prop | iOS | Android |
|---|---|---|
| `value`, `placeholder`, `editable` | | |
| `secureTextEntry` | `isSecureTextEntry` | `InputType` de contraseña |
| `color`, `fontSize`, `fontWeight`, `fontFamily` | `textColor`, `font` | `setTextColor`, `setTextSize`, `setTypeface` |
| `keyboardType` | `keyboardType` | `InputType` |
| `returnKeyType` | `returnKeyType` | `imeOptions` |
| `autoCapitalize` | `autocapitalizationType` | banderas de `InputType` |
| `autoCorrect` | `autocorrectionType` | `NO_SUGGESTIONS` |
| `placeholderColor` | `attributedPlaceholder` | `setHintTextColor` |
| `textAlign` | `textAlignment` | `gravity` |
| `[ios].clearButtonMode` | `clearButtonMode` | — |
| `[ios].borderStyle` | `borderStyle` | — |
| `[android].selectAllOnFocus` | — | `setSelectAllOnFocus` |
| `[android].cursorVisible` | — | `setCursorVisible` |

Salidas `(valueChange)` y `(submit)`. `(focus)` y `(blur)` vivían aquí y ahora
viven en la base, así que las tienen todos los primitivos.

**Se dejó fuera:** `maxLength`. Android lo hace con un `InputFilter` de una
línea; iOS no tiene nada para ello y hay que interceptar el
`shouldChangeCharactersInRange` del delegate, lo que significa poner un delegate
propio en un control que hoy solo lleva acciones. Se puede hacer; un `maxLength`
que solo funciona en Android es peor que no tenerlo.

### `an-text` — `UILabel` · `TextView` · `NSTextField`

| Prop | iOS | Android |
|---|---|---|
| `color`, `fontSize`, `fontWeight`, `fontStyle`, `fontFamily` | | |
| `textAlign`, `numberOfLines` | | |
| `lineHeight` | `NSParagraphStyle` | `setLineSpacing` |
| `letterSpacing` | `kern` de `NSAttributedString` | `setLetterSpacing` |
| `textDecoration` `none\|underline\|lineThrough` | atributos de texto | banderas de `Paint` |
| `[android].selectable` | — | `setTextIsSelectable` |

**Se dejó fuera:** `adjustsFontSizeToFitWidth`. `UILabel` lo tiene y `TextView`
tiene auto-dimensionado desde la API 26, así que sería una prop común legítima.
Lo que lo deja fuera es que el core mide el texto él mismo para el layout y no
sabe encoger, así que la caja seguiría siendo la grande. Hacerlo bien significa
tocar la medición, no el host.

### `an-switch` — `UISwitch` · `MaterialSwitch` · `NSSwitch`

| Prop | iOS | Android |
|---|---|---|
| `on`, `color` | `isOn`, `onTintColor` | `isChecked`, tinte de la pista |
| `thumbColor` | `thumbTintColor` | `setThumbTintList` |
| `[android].trackColor` | — | la pista apagada |

**Se dejó fuera:** el color de la pista apagada en iOS. `UISwitch` no lo expone.
Lo que circula es darle a un control del sistema un `backgroundColor` y un radio
de esquina de 16 para que el fondo se vea por detrás, lo que se rompe el día que
Apple cambie la altura del control. Ese es exactamente el apaño feo que este
proyecto no hace: en iOS se queda el color del sistema.

### `an-slider` — `UISlider` · `Slider` de Material · `NSSlider`

| Prop | iOS | Android |
|---|---|---|
| `value`, `minimumValue` (0), `maximumValue` (1), `color` | | |
| `minimumTrackColor` | `minimumTrackTintColor` | `setTrackActiveTintList` |
| `maximumTrackColor` | `maximumTrackTintColor` | `setTrackInactiveTintList` |
| `thumbColor` | `thumbTintColor` | `thumbTintList` |
| `[ios].continuous` | `isContinuous` | — |
| `[android].stepSize` | — | `setStepSize` |

Ojo, el máximo por defecto es **1**, no 100. El que llega a 100 es `an-stepper`.

**Se dejó fuera:** `stepSize` como prop común. `UISlider` es continuo y no tiene
pasos; redondear el valor en el host se puede, pero entonces el dedo va por un
lado y el valor por otro, y el control deja de dar la respuesta táctil que sí da
el de Android, que engancha de verdad. Prometer «pasos» y entregar dos
comportamientos distintos es peor que decir que solo Android los tiene.

### `an-scroll-view` — `UIScrollView` · `AnScrollView` · `NSScrollView`

| Prop | iOS | Android |
|---|---|---|
| `horizontal` | `alwaysBounce…`, un eje del `contentSize` | un `HorizontalScrollView` interior |
| `showsScrollIndicator` | `showsVertical…` | las dos barras |
| `refreshing` | `UIRefreshControl` | un arco nuestro |
| `bounces` | `bounces` | `overScrollMode` |
| `scrollEnabled` | `isScrollEnabled` | se traga el gesto |
| `[ios].pagingEnabled` | `isPagingEnabled` | — |
| `[ios].keyboardDismissMode` | `keyboardDismissMode` | — |

**Se dejó fuera:** los dos ejes a la vez. El core recorta el `contentSize` al
marco de la propia vista en el eje por el que no hace scroll, así que una vista
con scroll ofrece una dirección y solo una. Android es el motivo de que siga
así: `ScrollView` y `HorizontalScrollView` son dos clases, y las dos anidadas se
pelean por cada arrastre en diagonal.

### `an-tab-bar` — `UITabBarController` · `AnTabBar` · `NSSegmentedControl`

| Prop | iOS | Android |
|---|---|---|
| `items`, `icons`, `selectedIndex`, `color` | | |
| `unselectedColor` | `unselectedItemTintColor` | color inactivo |
| `[ios].translucent` | `isTranslucent` | — |

`items` e `icons` viajan como cadenas JSON.

**Se dejó fuera:** las insignias. En iOS es `UITabBarItem.badgeValue` y sale
gratis; en Android la barra es nuestra —la plataforma no trae ninguna— así que
la burbuja habría que dibujarla a mano, y una burbuja dibujada a mano al lado de
una del sistema no se parecen. Anotado como trabajo de la tab bar de Android, no
del envoltorio.

### `an-select` — `UIButton` + `UIMenu` · `Spinner` · `NSPopUpButton`

Solo `items` y `selectedIndex`, más el `enabled` heredado.

**Se dejó fuera:** `mode` (`dropdown` o `dialog`). El `Spinner` de Android lo
decide en su constructor y no se puede cambiar después, y iOS no tiene las dos
formas en absoluto — un `UIMenu` siempre es un menú. Soportarlo significaría
destruir y reconstruir la vista sobre la marcha, que es precisamente lo que
evita el árbol.

### El resto

`an-view` no añade absolutamente nada — es la demostración pura de lo que te da
la base.

| Primitivo | Sus props propias |
|---|---|
| `an-stack-view` | `transition` `push\|pop\|none`; salida `(back)` |
| `an-image` | `source`, `resizeMode`, `intrinsicWidth`, `intrinsicHeight`; salida `(load)` |
| `an-icon` | `name`, `size` (24), `weight`, `color` |
| `an-textarea` | `value`, `editable`, `color`; salida `(change)` |
| `an-segmented-control` | `items`, `selectedIndex`, `color`; salida `(change)` |
| `an-stepper` | `value`, `minimumValue`, `maximumValue` (100), `step`; salida `(change)` |
| `an-search-bar` | `value`, `placeholder`; salidas `(input)`, `(submit)` |
| `an-date-picker` | `value` (una `Date` o ms), `mode` `date\|time\|dateAndTime`; salida `(change)` |
| `an-navigation-bar` | `title`, `showsBack`, `backTitle`; salida `(back)` |
| `an-progress-bar` | `progress`, `color` |
| `an-activity-indicator` | `animating`, `color` |
| `an-modal` | `visible`, `presentation` `fullScreen\|sheet`, `[ios].detents`; salida `(dismiss)` |
| `an-alert` | `visible`, `title`, `message`, `buttons`, `sheet`; salida `(select)` |
| `an-web-view` | `url`, `html` |
| `an-map-view` | `latitude`, `longitude`, `zoom` (12), `showsUser` |
| `an-video-view` | `url`, `playing`, `muted` |

`an-safe-area` no está en esta lista porque no es un primitivo: es un componente
construido sobre `an-view` que convierte los insets que informa el host en
padding. Mira [Componentes](/es/reference/components/).

## La comprobación que mantiene esto honesto

`scripts/check-wrapper.sh` compara las props que declaran las directivas con lo
que leen los hosts, y suspende la compilación si no cuadran. Los cuatro hosts, no
los dos teléfonos: iOS, Android, macOS y watchOS. Y lee el crate **entero** cada
vez, no un fichero — las props de accesibilidad de Apple viven en su propio
módulo, y una comprobación que solo mirara el fichero principal del host
informaría de las seis como inalcanzables.

Mantiene cuatro listas, y las cuatro son el asunto:

- **Solo del core.** `intrinsicWidth` e `intrinsicHeight`. El layout las consume
  para reservar el sitio de una imagen que todavía no ha cargado, y no tienen
  nada que decirle a ningún host.
- **Solo de puntero.** `cursor`. La forma de un puntero solo significa algo
  donde hay puntero, y un dedo no tiene forma. Exigir que iOS y Android la lean
  sería exigirles algo que no pueden hacer, y ponerla en una lista de pendientes
  daría a entender que algún día lo harán. Así que la misma exigencia se le hace
  al host de **escritorio**, con la misma dureza. Su salida hermana, `(hover)`,
  no está aquí porque las salidas no son props: viajan por otro camino, y allí
  cada host dice lo que no puede entregar. Mira [macOS](/es/platforms/macos/).
- **Pendientes.** Props que deberían llegar y no llegan a algún host. **Ahora
  mismo está vacía**, y la lista solo puede encoger: una prop que siga en ella y
  que ahora sí llegue a todos los hosts también es un fallo, así que nadie puede
  dejar una fuga arreglada marcada como rota.
- **Pendientes en el reloj.** Trece props que llegan a iOS, Android y macOS y se
  paran en watchOS: `autoCapitalize`, `autoCorrect`, `bounces`, `icon`,
  `iconPosition`, `lineHeight`, `maximumTrackColor`, `minimumTrackColor`,
  `placeholderColor`, `refreshing`, `returnKeyType`, `thumbColor` y `variant`.
  `an-watch` no tiene jerarquía de vistas: refleja el árbol en un modelo que
  SwiftUI redibuja, así que una prop solo llega si `snapshot.rs` la copia a ese
  modelo y el shell la lee de vuelta, y estas trece no se copian. Ninguna es una
  negativa —watchOS puede con las trece— y la lista solo puede encoger con la
  misma regla que la de arriba. Una prop que solo viaja en un primitivo que el
  reloj no monta (`an-web-view`, `an-map-view`, `an-video-view`,
  `an-navigation-bar`, `an-tab-bar`) no está en ella: el reloj no la está
  perdiendo, es que allí no hay nada sobre lo que ponerla. Cuáles son esos
  primitivos se lee de la lista de negativas del propio host del reloj, para que
  las dos no puedan separarse.

Hay además una comprobación estructural: el número de inputs `[ios]`/`[android]`
declarados tiene que ser igual al número de veces que se llama al ayudante que
los desmonta, para que nadie pueda colar un objeto de plataforma con una
asignación normal y perder una clave desconocida en silencio.

Que la tercera lista esté vacía es la respuesta actual a «¿esta prop hace algo
de verdad?» en los tres hosts que montan vistas: **no hay fugas conocidas en
iOS, Android ni macOS**, y quedan trece abiertas en el reloj. Llegó ahí
encontrando ocho — una
contraseña dibujada en texto plano en Android, un spinner que no paraba nunca,
una barra de scroll que no se podía esconder, un rebote que no se podía apagar,
dos props de fuente que no hacían nada, y `lineHeight` y `letterSpacing`, que
fueron las peores: el core *medía* con ellas y el host dibujaba sin ellas, así
que el layout reservaba sitio para un texto con más interlineado que el que se
pintaba.
