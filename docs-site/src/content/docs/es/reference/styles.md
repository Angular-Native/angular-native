---
title: Estilos y layout
description: El subconjunto de flexbox que resuelve el núcleo de Rust, los valores que acepta, y qué hace un nombre de estilo que no conoce.
sidebar:
  order: 2
---

El layout no es de la plataforma. Flexbox corre una vez, en el núcleo, sobre
[taffy](https://github.com/DioxusLabs/taffy), y a cada host se le entregan frames
absolutos en puntos lógicos. Así que el juego de estilos de abajo es el mismo en
un móvil, en una tele y en un reloj: una plantilla que se coloca de una manera se
coloca así en todas partes.

**No hay cascada, ni herencia, ni selectores, ni hojas de estilo.** Un estilo se
escribe en el nodo al que se aplica:

```html
<an-view [style.flexDirection]="'row'" [style.gap]="'12'" [style.padding]="'16'">
  <an-view [style.flex]="'1'"><an-text>izquierda</an-text></an-view>
  <an-view [style.width]="'80'"><an-text>derecha</an-text></an-view>
</an-view>
```

Los valores son cadenas porque es lo que produce el binding `[style.x]` de
Angular. El núcleo los parsea una vez, al asignarlos.

## Lo que acepta el núcleo

51 nombres, y nada más. Están declarados en `crates/an-layout/src/style.rs` y
duplicados en el lado de TypeScript en
`packages/platform-native/src/style-names.ts`; `scripts/check-styles.sh` falla si
las dos listas se separan.

### Caja y flujo

| Estilo | Valores | Por defecto |
|---|---|---|
| `display` | `flex`, `none` | `flex` |
| `position` | `relative`, `absolute` (`static` se lee como relative, `fixed` como absolute) | `relative` |
| `overflow` | `visible`, `hidden`, `scroll` | `visible` |
| `boxSizing` | `border-box`, `content-box` | `border-box` |

`overflow` pone los dos ejes a la vez: no hay `overflowX`.

### Qué hace recortar, exactamente

`hidden` y `scroll` **recortan**: la caja del nodo es todo lo que puede pintar, y
un hijo que se sale se corta en el borde en vez de dibujarse encima de lo que
tenga al lado. `visible`, el valor por defecto, lo deja pasar.

Dos nodos recortan diga lo que diga la plantilla:

- **La raíz.** Es el cuerpo de la app. Nada de lo que contenga puede pintarse
  fuera de la ventana: un cuerpo que no recorta es un lienzo, y el contenido
  empujado más allá del borde sigue existiendo fuera de la pantalla.
- **Todo lo que tenga scroll.** Si no, su contenido se pinta encima de lo que lo
  rodea en cuanto hay más del que cabe.

:::caution[Dos hosts solo aprietan]
En Android y en watchOS los contenedores han recortado siempre sin condiciones, y
`overflow: visible` nunca ha funcionado ahí. Así que lo que llega del layout solo
puede **activar** el recorte, nunca desactivarlo: obedecer `visible` ahí no sería
respetar un estilo, sería una funcionalidad nueva colada como efecto secundario
de un arreglo, y todos los layouts hechos contra el comportamiento anterior
empezarían a soltar hijos por fuera. En iOS, tvOS, visionOS y macOS se aplica el
valor resuelto tal cual.
:::

### Nada hace scroll horizontal

`contentSize` se calcula asumiendo que el desbordamiento va hacia abajo, así que
el contenido de un nodo con scroll se **acota a su propio ancho** — en el núcleo,
y otra vez en el host, que es donde ocurre el scroll.

Esa cota no es pulcritud. Un `UIScrollView` hace scroll en el eje en el que su
contenido sea mayor, así que un contenido unos puntos más ancho —una imagen que
reporta su tamaño intrínseco dentro de una fila, por ejemplo— convierte la página
entera en algo que se puede arrastrar de lado hasta dejar la pantalla vacía. En
vertical el desbordamiento es el objetivo; en horizontal es siempre un error en
otro sitio, y debe verse como un borde recortado y no como una pantalla que se
puede barrer.

`scripts/check-clip.sh` comprueba las dos cosas: que la raíz y todo nodo con
scroll recortan, que un contenedor normal no, y que ningún nodo con scroll es más
ancho por dentro que por fuera.

### Flex

| Estilo | Valores | Por defecto |
|---|---|---|
| `flexDirection` | `row`, `column`, `row-reverse`, `column-reverse` | **`column`** |
| `flexWrap` | `nowrap`, `wrap`, `wrap-reverse` | `nowrap` |
| `justifyContent` | `flex-start`, `flex-end`, `center`, `space-between`, `space-around`, `space-evenly`, `stretch` | sin poner |
| `alignItems` | `flex-start`, `flex-end`, `center`, `stretch`, `baseline` | sin poner |
| `alignSelf` | igual que `alignItems` | sin poner |
| `alignContent` | igual que `justifyContent` | sin poner |
| `flex` | un número | — |
| `flexGrow` | un número | `0` |
| `flexShrink` | un número | **`0`** |
| `flexBasis` | puntos, porcentaje o `auto` | `auto` |

Dos de esos valores por defecto son los de React Native y no los de CSS, y los
dos importan: los hijos se apilan **hacia abajo** salvo que se diga otra cosa, y
nada encoge por debajo de su tamaño salvo que se pida.

`flex: N` es la forma corta y significa lo que significa en CSS: crecer `N`,
encoger `1`, base `0`. Es lo que casi todo el mundo escribe en vez de las tres por
separado.

`justifyContent` y `alignContent` aceptan el mismo juego de palabras clave;
`alignItems` y `alignSelf` aceptan `baseline` pero no la familia `space-*`.

### Tamaño

| Estilo | Valores |
|---|---|
| `width`, `height` | puntos, porcentaje, `auto` |
| `minWidth`, `minHeight`, `maxWidth`, `maxHeight` | puntos, porcentaje, `auto` |
| `aspectRatio` | un número — ancho dividido entre alto |

Un máximo sin poner es `auto`, no cero. Un `maxWidth` de cero dejaría la vista sin
tamaño ninguno, que es justo lo contrario de «no hay máximo».

### Espaciado

| Estilo | Valores |
|---|---|
| `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft` | puntos, porcentaje, `auto` |
| `marginHorizontal`, `marginVertical` | lo mismo, dos bordes de una vez |
| `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft` | puntos, porcentaje |
| `paddingHorizontal`, `paddingVertical` | lo mismo, dos bordes de una vez |
| `gap`, `rowGap`, `columnGap` | puntos, porcentaje |

`marginStart` y `marginEnd` se aceptan como formas de escribir `marginLeft` y
`marginRight`, y lo mismo con el padding. **No** son sensibles a la dirección:
esto no es un motor de layout de derecha a izquierda, y los nombres se admiten
solo para que una plantilla escrita con ellos no se quede sin hacer nada en
silencio.

Los márgenes admiten `auto`; el padding y el gap, no.

### Grosor de borde y desplazamientos

| Estilo | Valores |
|---|---|
| `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth` | puntos, porcentaje |
| `top`, `right`, `bottom`, `left` | puntos, porcentaje, `auto` |

El **grosor** del borde es layout, así que vive aquí. El color y el radio no lo
son: son props de la primitiva, junto con `backgroundColor`. Ver
[Componentes](/es/reference/components/).

## Valores

| Escrito | Se lee como |
|---|---|
| `'16'`, `'16px'` | 16 puntos lógicos |
| `'50%'` | la mitad de la dimensión correspondiente del padre |
| `'auto'` | el comportamiento automático de la prop |
| `'row'`, `'center'`, … | una palabra clave, donde la prop admita una |
| `''` | sin poner — vuelta al valor por defecto |

Las unidades son **puntos lógicos**, no píxeles físicos. El factor de escala del
dispositivo no entra nunca en una plantilla; es asunto de la plataforma cuando
pinta.

Las dos formas de escribir cada palabra clave con guion valen: `space-between` y
`spaceBetween`, `flex-start` y `flexStart` (y `start`), `column-reverse` y
`columnReverse`. Angular pone guiones en los *nombres* de estilo antes de
entregarlos, así que `[style.flexDirection]` y `[style.flex-direction]` son lo
mismo para el núcleo.

Un valor que el núcleo no sepa parsear se convierte en `Unset`: la prop vuelve a
su valor por defecto y no a lo que tuviera antes.

## Los estilos de fuente no son estilos

Nueve nombres parecen de CSS pero son **props**, no layout:

`fontSize` · `fontWeight` · `fontStyle` · `fontFamily` · `lineHeight` ·
`letterSpacing` · `color` · `textAlign` · `numberOfLines`

El núcleo los necesita para *medir* el texto y el host para *pintarlo*, así que
viajan como props del nodo. Escribirlos como `[style.fontSize]` funciona —el
renderer reconoce los nueve y los reencamina— pero el input tipado `[fontSize]`
es el que puede comprobar el compilador de plantillas, y es el que hay que
escribir.

Antes de que existiera ese reencaminamiento, `[style.fontSize]` no hacía
absolutamente nada: el texto se medía con la fuente por defecto y se pintaba con
la de UIKit, así que se salía de una caja dimensionada para algo más pequeño y el
padre lo recortaba. Parecía texto desapareciendo.

## Un estilo desconocido lo dice

Angular acepta `[style.loquesea]` sin rechistar. Así que el renderer comprueba el
nombre contra la lista de arriba y avisa, **una vez por nombre**, cuando nadie lo
va a mirar: nombra el estilo, dice que no va a hacer nada, y apunta a dónde
probablemente pertenecía — una propiedad del control, como un color, un título o
un valor, es un input tipado y no un estilo.

Una vez por nombre y no una vez por escritura: el mismo estilo se vuelve a poner
en cada pasada de detección de cambios, y avisar cada vez llenaría el log sin
decir nada nuevo.

Esta es la forma que toma todo el framework ante un hueco. Un estilo que viaja,
que ningún host lee y que no levanta nada es el tipo de bug más caro que hay:
parece que la funcionalidad sencillamente no va, y no hay nada que buscar con un
grep.

## Clases y `!important`

`addClass` acumula nombres y los manda como una prop `className`. Nada en el
núcleo ni en los hosts las resuelve —no hay hojas de estilo— así que una clase es
inerte salvo que una capa de estilado por encima haga algo con ella.

`RendererStyleFlags2.Important` se ignora y el valor se aplica. Sin cascada no hay
nada contra lo que `!important` pueda ganar.

## La medición: de dónde sale un tamaño cuando no lo das

Un nodo sin tamaño explícito se mide preguntando a la plataforma, a través del
trait `TextMeasurer`. Hay tres tipos de hoja:

- **Texto** — medido con la tipografía real del sistema, con las props de fuente
  de arriba, contra el ancho disponible. El mínimo intrínseco se pregunta aparte:
  pedirlo como «ancho disponible cero» parece equivalente y no lo es — las dos
  plataformas contestan cero, y entonces el texto se encoge hasta desaparecer en
  cuanto nadie le impone un ancho.
- **Controles** — un `UISwitch`, un `MaterialButton`. Cada uno se mide una vez al
  arrancar a partir de un control de muestra, porque crear uno necesita el hilo
  principal y el medidor vive en el del motor.
- **Imágenes** — a partir del tamaño intrínseco, conservando la relación de
  aspecto cuando solo una dimensión está fijada.

Un nodo con función de medida es una **hoja**: el layout no baja por debajo de
él, tenga hijos o no.

Esa medición del arranque conviene tenerla presente cuando a un control se le pide
que sostenga más de lo que sostenía la muestra. Un botón con un `[ios].subtitle`
ocupa dos líneas y la muestra ocupaba una, así que necesita un alto explícito en
la plantilla o el título se recorta.

## `contentSize`, y el eje que falta

Para un nodo con scroll el núcleo reporta además cuánto sitio ocupan los hijos,
que es lo que un `UIScrollView` quiere como su `contentSize`.

Lo calcula asumiendo que el desbordamiento va **hacia abajo**. Así que el scroll
horizontal no es una prop de host que falte: es trabajo en `an-core`.
