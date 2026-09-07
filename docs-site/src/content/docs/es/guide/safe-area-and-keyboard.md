---
title: El área segura y el teclado
description: Un solo evento reporta todos los márgenes que reserva el sistema —muesca, barras, bisel y el teclado— y llega en movimiento, una vez por frame, así que el layout viaja con la animación en vez de llegar antes que ella.
sidebar:
  order: 8
---

Cada plataforma se guarda partes de la pantalla para sí: una muesca, una barra de
estado, un indicador de inicio, la barra de navegación de Android, el bisel
redondo de un reloj. Ninguno es constante y ninguno se puede calcular por
adelantado: cambian al rotar, al entrar en pantalla partida y cuando sale el
teclado.

El host los mide y los reporta todos por un mismo evento:

```html
<an-view (safeArea)="onInsets($event)"></an-view>
```

```ts
onInsets(insets: NativeSafeAreaInsets) {
  // { top, right, bottom, left }, en puntos
}
```

Casi siempre no quieres el evento, quieres el padding. Para eso está
`an-safe-area`.

## `<an-safe-area>`

```html
<an-safe-area [edges]="['top', 'bottom']" [padding]="20" [style.gap]="'12'">
  <an-text>ya no está debajo de la muesca</an-text>
</an-safe-area>
```

```ts
import { SafeArea } from '@angular-native/primitives'
```

| Prop | Tipo | Por defecto |
|---|---|---|
| `edges` | `('top' \| 'right' \| 'bottom' \| 'left')[]` | los cuatro |
| `padding` | `number` | `0` — padding tuyo, **sumado a** lo que reserva el sistema |

Dos decisiones que lleva dentro merecen conocerse, porque las dos fueron bugs
antes.

**No hay ninguna vista dentro.** El área segura *es* su vista, así que lo que le
pongas para colocar a sus hijos —`gap`, `flexDirection`, `alignItems`— los
gobierna. Con una vista intermedia no lo hacía: los estilos se quedaban en la de
fuera, que tenía exactamente un hijo, y no pasaba nada. Ni ruido, ni error, ni
espaciado.

**`padding` es una prop y no `[style.padding]`.** El padding de esta vista ya lo
escribe el área segura; poner los dos haría que se pisaran, y el que perdiera
perdería en silencio.

:::note[Padding, no margen — y no por preferencia]
Los márgenes que reserva el sistema se miden *para la vista a la que pertenecen*.
Una vista que se apartara con un margen dejaría de estar bajo la muesca,
empezaría por tanto a reservar cero, volvería a donde estaba, estaría otra vez
bajo la muesca — y así para siempre. El padding no mueve la vista, así que la
medición se mantiene estable.
:::

## El teclado va en el margen inferior

No es un evento aparte y no es una suma. Mientras el teclado está subido se pinta
**encima** del indicador de inicio y de la barra de navegación, así que lo que
llega en `bottom` es el mayor de los dos, no su total.

La consecuencia práctica es corta: un formulario cuyo último campo acabaría
debajo del teclado no necesita más que `'bottom'` entre sus bordes.

```html
<an-safe-area [edges]="['top', 'bottom']" [style.flexGrow]="'1'">
  <an-scroll-view [style.flexGrow]="'1'">
    <!-- el último campo sigue siendo alcanzable cuando se abre el teclado -->
  </an-scroll-view>
</an-safe-area>
```

Y simétricamente: un área segura que **no** lista `'bottom'` está pidiendo que no
se la mantenga libre por abajo, teclado incluido. Es una petición legítima —un
fondo a sangre, un vídeo—, solo conviene saber que la has hecho.

## Llega en movimiento

El margen no salta a su valor final cuando se abre el teclado. Se entrega **una
vez por frame**, interpolado a donde está el teclado de verdad, así que el layout
viaja con él en vez de plantarse antes.

- **En iOS**, la duración y la curva salen de la notificación del teclado, que es
  la misma información que usan las animaciones del propio UIKit.
- **En Android** es `WindowInsetsAnimation.Callback`, cuyo `onProgress` se llama
  una vez por frame con los márgenes interpolados.

Ahí aterriza todo lo que el teclado sabe hacer: abrirse, cerrarse, apartarse
arrastrado con un dedo, cambiar de tamaño porque cambió el idioma de entrada, y
todo otra vez tras una rotación o un arrastre a pantalla partida.

## El hueco de Android, dicho y no disimulado

`WindowInsets.Type.ime()` no existe antes de **API 30**, y
`WindowInsetsAnimation.Callback` tampoco. Así que:

| Nivel de API | Qué llega |
|---|---|
| 30 y superior | El margen del teclado, una vez por frame, siguiendo la animación. |
| 24 – 29 | **Nada.** Un campo abajo se queda debajo del teclado. |

El truco que existía antes de Android R era mirar cómo encogía el marco visible
de la ventana, y solo reporta algo cuando se permite que la ventana se
redimensione. Este shell pide que **no** se redimensione — `setDecorFitsSystemWindows(false)` —
precisamente para que el layout que calculó el núcleo sea el que se pinta.
Volver a meter el truco antiguo significaría dos modelos de layout corriendo en
una pantalla, que es peor cosa que tener a tu nombre que un hueco documentado en
niveles de API para los que Play no acepta subidas desde 2024.

Las barras y el recorte de pantalla van bien desde API 24; lo único que falta es
el teclado.

## Por plataforma

| | De dónde salen los márgenes |
|---|---|
| **iOS · iPadOS** | `UIView.safeAreaInsets` —muesca o isla dinámica, barra de estado, indicador de inicio— más el teclado, que UIKit **no** pone en `safeAreaInsets` y que añade este host. |
| **tvOS · visionOS** | El mismo host y el mismo `safeAreaInsets`, sea lo que sea lo que esa plataforma reporte para la escena. |
| **Android · Wear OS** | `WindowInsets` — barra de estado, barra de navegación, recorte de pantalla, y desde API 30 el teclado. En un reloj redondo es por donde llega el margen del bisel, y es la diferencia entre una lista legible y texto cortado por la curva. |
| **macOS** | Cero por los cuatro lados, contestado una vez al suscribirse en vez de dejar la plantilla esperando. El área de contenido de una ventana ya es el área de contenido. |
| **watchOS** | No llega nada, y al suscribirse se dice: *«una app de reloj ocupa la pantalla entera y el sistema no reserva márgenes que se puedan preguntar»*. `an-safe-area` se monta y se queda a cero. |

:::note[El teclado no está en `safeAreaInsets`]
En iOS esos márgenes son los recortes de la pantalla y nada más; UIKit reporta el
teclado por una notificación. Un framework que solo lea `safeAreaInsets` deja un
campo al final de un formulario debajo del teclado, y por eso este host funde los
dos antes de reportar.
:::

## Por qué los primeros márgenes pueden ser cero

Llegan **después** de que se hayan montado las primeras vistas. Nadie iría a
pedirlos de nuevo, así que el shell de Android registra también el
`setOnApplyWindowInsetsListener` normal además del callback de animación — sin
eso, la muesca se reportaba como cero siempre que el bundle ganaba la carrera,
cosa que hace en una máquina cargada.

Si estás leyendo `(safeArea)` a mano en vez de usar `an-safe-area`, espera que el
primer valor sean ceros y que llegue otro poco después. El componente ya lo
contempla: arranca a cero y se vuelve a pintar cuando aparecen los números
buenos.

## El ejemplo

```bash
cargo an dev examples/measure
```

`scripts/check-measure.sh` cubre la parte de layout sin dispositivo;
`scripts/check-keyboard-device.sh` es la mitad que necesita un móvil de verdad,
porque la altura del teclado es la respuesta de la plataforma y no algo de lo que
uno pueda fiarse en un simulador.
