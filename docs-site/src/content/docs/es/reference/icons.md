---
title: Iconos
description: SF Symbols en las plataformas de Apple y Material Symbols en Android, pedidos por nombre — el vocabulario compartido, cuándo escribir el nombre nativo, y por qué uno de los dos va empaquetado y el otro no.
sidebar:
  order: 6
---

```html
<an-icon [name]="'settings'" [size]="28" [color]="'#f4f7ff'" />
```

Eso es un SF Symbol en iOS, en la tele, en el visor y en el reloj, y un Material
Symbol en Android y en Wear OS. Aquí no se dibuja nada ni se calca ningún icono:
el nombre va al sistema y el sistema entrega el glifo con el trazo y el peso que
esa versión del sistema operativo le dé — lo que significa que el icono envejece
con la plataforma en vez de quedarse anclado al día en que entró en el proyecto.

## Las props

| Prop | Tipo | Por defecto | |
|---|---|---|---|
| `name` | `string` | — | Un nombre compartido, o el de la plataforma. |
| `size` | `number` | `24` | Puntos. También **fija el ancho y el alto de la vista**. |
| `weight` | `number` | sin poner | El trazo, en la escala tipográfica: `100`–`900`. |
| `color` | `string` | sin poner | |

`size` no es solo un tamaño. En las plataformas de Apple un símbolo grande no es
el pequeño escalado, es otro dibujo, así que el número se le pasa a
`UIImageSymbolConfiguration` y el sistema elige el trazo que le corresponde.
Además se escribe como estilos `width` y `height`, porque el layout tiene que
saber cuánto ocupa la caja — sin eso un `<an-icon>` sin nada más se mediría a
cero y no aparecería nunca.

`weight` se mapea a la escala de la plataforma:

| `weight` | Peso del SF Symbol |
|---|---|
| `100`–`299` | Light |
| `300`–`499` | Regular |
| `500`–`599` | Medium |
| `600`–`699` | Semibold |
| `700` y más | Bold y por encima |

## Un nombre, dos juegos de iconos

Treinta y dos nombres comunes —los que lleva casi cualquier app en una barra de
pestañas o en una cabecera— se traducen al de cada plataforma, de forma que la
misma plantilla vale en las dos:

| Nombre | SF Symbol | Material Symbol |
|---|---|---|
| `home` | `house.fill` | `home` |
| `search` | `magnifyingglass` | `search` |
| `settings` | `gearshape.fill` | `settings` |
| `profile` · `account` | `person.crop.circle.fill` | `account_circle` |
| `back` | `chevron.left` | `arrow_back` |
| `forward` | `chevron.right` | `arrow_forward` |
| `close` | `xmark` | `close` |
| `add` | `plus` | `add` |
| `remove` | `minus` | `remove` |
| `delete` | `trash` | `delete` |
| `edit` | `pencil` | `edit` |
| `share` | `square.and.arrow.up` | `share` |
| `favorite` | `heart.fill` | `favorite` |
| `star` | `star.fill` | `star` |
| `menu` | `line.3.horizontal` | `menu` |
| `more` | `ellipsis` | `more_horiz` |
| `check` | `checkmark` | `check` |
| `info` | `info.circle` | `info` |
| `warning` | `exclamationmark.triangle.fill` | `warning` |
| `refresh` | `arrow.clockwise` | `refresh` |
| `calendar` | `calendar` | `calendar_month` |
| `camera` | `camera.fill` | `photo_camera` |
| `bell` | `bell.fill` | `notifications` |
| `chat` | `bubble.left.fill` | `chat_bubble` |
| `mail` | `envelope.fill` | `mail` |
| `list` | `list.bullet` | `list` |
| `play` | `play.fill` | `play_arrow` |
| `pause` | `pause.fill` | `pause` |
| `download` | `arrow.down.circle` | `download` |
| `upload` | `arrow.up.circle` | `upload` |
| `location` | `location.fill` | `location_on` |
| `lock` | `lock.fill` | `lock` |

El vocabulario usa la nomenclatura de Material, que es por lo que el lado Android
solo tiene que traducir los diez que de verdad se diferencian y deja pasar el
resto tal cual.

## La tabla es un atajo, no una lista blanca

**Lo que no esté en ella pasa sin tocarse.** Esa es la mitad importante:

```html
<an-icon [name]="'figure.run'" />        <!-- un SF Symbol, escrito directamente -->
<an-icon [name]="'directions_run'" />    <!-- un Material Symbol, escrito directamente -->
```

Hay más de cinco mil SF Symbols y otros tantos Material Symbols. Duplicar
cualquiera de las dos listas aquí no tendría sentido, y convertiría un atajo en
una puerta. Los treinta y dos son los que merece la pena no escribir dos veces;
todo lo demás es una línea en cada una de dos plantillas, o un mapa pequeño
propio.

Un nombre que el juego de esa plataforma no tenga no pinta **nada**, en silencio:
iOS no recibe ningún `UIImage` y Android no encuentra ningún codepoint, y ninguno
de los dos lo registra en el log. La caja sigue ahí —`size` le dio ancho y
alto—, simplemente está vacía. Si un icono falta en una plataforma y en la otra
no, lo primero que hay que mirar es una errata o un nombre que solo existe en uno
de los dos juegos.

## En Apple no va nada empaquetado. En Android sí.

**En las plataformas de Apple** el símbolo es `UIImage.systemImageNamed`. El
juego es parte del sistema, crece con cada versión y ya respeta el tamaño de
texto de accesibilidad del usuario. La app no lleva ningún recurso de iconos.

**En Android** la fuente de Material Symbols **sí** va empaquetada —
`material-symbols.ttf` y su tabla de codepoints, en los assets del shell — y el
icono se pinta como un glifo a través de un `Typeface`.

Esa asimetría no es una preferencia. Lo que Android trae en la plataforma está
congelado desde 2011: `android.R.drawable` es el juego de iconos de la 2.x, y no
es el de Material 3. Una app que lo usara parecería una app de otra década al
lado del interruptor y del botón que tiene al lado, que sí **son** Material 3.
Así que el juego actual viaja con la app.

Es el único sitio de este proyecto en el que algo se lleva encima en vez de
pedirse, y se lleva encima por la misma razón por la que todo lo demás se pide:
para que el resultado sea lo que la plataforma pinta hoy.

:::note[Donde el icono es un `Drawable`, no una vista]
El `[icon]` de `an-button` pasa por la misma tabla, pero el `MaterialButton` de
Android quiere un `Drawable` y no un glifo, así que la fuente se rasteriza a uno.
El nombre que escribes es el mismo en los dos casos.
:::

## La tabla vive en tres sitios

`crates/an-core/src/icons.rs` es la copia compartida, `crates/an-ios/src/icons.rs`
tiene una para los hosts de UIKit, y `AnHost.java` tiene las diez divergencias de
Android. A diferencia de los nombres de estilo y de los de las primitivas,
**ningún script compara las tres** — en
[comprobarlo sin dispositivo](/es/guide/testing/#las-listas-que-no-pueden-separarse)
está cómo es esa guardia donde sí existe.

Si añades un nombre común, añádelo en los tres.
