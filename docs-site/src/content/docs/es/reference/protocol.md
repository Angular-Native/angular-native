---
title: El protocolo binario
description: Doce opcodes, little-endian, un búfer por tick — el formato de cable entre JavaScript y el núcleo de Rust, los códigos de nodo, y qué pasa cuando un comando se rechaza.
sidebar:
  order: 8
---

JavaScript no llama a Rust una vez por mutación. Escribe comandos en un búfer y
entrega el búfer entero al final del tick.

La razón es aritmética. Un `@for` sobre 200 filas son unas 1.200 mutaciones. Una
llamada por cada una son 1.200 cruces de frontera; un búfer es uno.

Casi nadie llega a esta página: `Renderer2` escribe el búfer y el núcleo lo lee.
Está aquí para quien esté depurando una mutación que no llegó, escribiendo un
host, o preguntándose qué quiere decir «el protocolo» cuando otra página dice que
un nombre está congelado por él.

## La forma

Todo little-endian. Cada comando es un byte de opcode seguido de sus operandos.
Las cadenas llevan una longitud `u32` en bytes y después UTF-8, **sin
alineación**: el decodificador lee byte a byte y no la necesita.

| Tipo | Bytes |
|---|---|
| opcode | `u8` |
| id de nodo | `u32` |
| índice | `u32` |
| número | `f64` |
| booleano | `u8`, cero o no |
| cadena | longitud `u32`, y después esos bytes en UTF-8 |

## Los doce opcodes

| Código | Comando | Operandos |
|:--:|---|---|
| `0x01` | `CREATE_NODE` | id `u32`, kind `u8` |
| `0x02` | `DESTROY_NODE` | id `u32` |
| `0x03` | `INSERT_CHILD` | padre `u32`, hijo `u32`, índice `u32` |
| `0x04` | `REMOVE_CHILD` | padre `u32`, hijo `u32` |
| `0x05` | `SET_STYLE` | id `u32`, nombre `str`, valor `str` |
| `0x06` | `SET_PROP_STR` | id `u32`, clave `str`, valor `str` |
| `0x07` | `SET_PROP_NUM` | id `u32`, clave `str`, valor `f64` |
| `0x08` | `SET_PROP_BOOL` | id `u32`, clave `str`, valor `u8` |
| `0x09` | `SET_PROP_NULL` | id `u32`, clave `str` |
| `0x0A` | `SET_TEXT` | id `u32`, texto `str` |
| `0x0B` | `SET_LISTENER` | id `u32`, evento `str`, activado `u8` |
| `0x0C` | `SET_ROOT` | id `u32` |

Los estilos viajan como cadenas y los parsea el núcleo al asignarlos, porque eso
es lo que produce el binding `[style.x]` de Angular. Las props van tipadas en el
cable —cuatro opcodes en vez de uno— para que un número no haya que volver a
parsearlo al otro lado, y para que `null` signifique *sin poner* y no la cadena
`"null"`.

`SET_LISTENER` es lo que hace que un gesto no cueste nada hasta que se pide:
llega cuando Angular se suscribe a un output, y el host engancha el reconocedor
entonces.

## Los ids de nodo los asigna JavaScript

Crear un nodo no necesita ida y vuelta. JS elige el id, escribe `CREATE_NODE` y
sigue refiriéndose a él — igual que los tags de Fabric. No hay ninguna llamada de
reserva que esperar ni ninguna tabla de ids que reconciliar.

## Los tipos de nodo

| Código | Tipo | Código | Tipo |
|:--:|---|:--:|---|
| `0` | `View` | `13` | `Modal` |
| `1` | `Text` | `14` | `Alert` |
| `2` | `RawText` | `15` | `Icon` |
| `3` | `Image` | `16` | `SegmentedControl` |
| `4` | `ScrollView` | `17` | `Stepper` |
| `5` | `TextInput` | `18` | `SearchBar` |
| `6` | `StackView` | `19` | **`Picker`** |
| `7` | `TabBar` | `20` | `DatePicker` |
| `8` | `Switch` | `21` | `NavigationBar` |
| `9` | `Slider` | `22` | **`TextEditor`** |
| `10` | `ActivityIndicator` | `23` | `WebView` |
| `11` | `ProgressBar` | `24` | `MapView` |
| `12` | `Button` | `25` | `VideoView` |

`RawText` no se escribe en ninguna plantilla: no tiene ni etiqueta ni directiva.
Es el nodo de texto que crea Angular para los caracteres que van dentro de un
`<an-text>`.

Los dos en negrita son los nombres que el núcleo no puede cambiar. `an-select`
viaja como `Picker` y `an-textarea` como `TextEditor` — elegidos cuando la
etiqueta no se podía llamar `Select` ni `TextArea`, porque Angular no autocierra
nada que se llame como un elemento de HTML. El prefijo les devolvió después el
nombre a las etiquetas, pero renombrar el enum significaría cambiar el protocolo,
así que el cable conserva los antiguos.

En todo lo demás el nombre sale de una regla y no de una tabla: se le quita `an-`
y se une en PascalCase, así que `an-text-input` es `TextInput`. Esa regla cruza
cuatro ficheros en tres lenguajes, y `scripts/check-kinds.sh` es lo que impide
que se separen.

## Cuando un comando se rechaza

`apply()` devuelve cuántos comandos aplicó, o uno de cinco errores:

| Error | Qué significa |
|---|---|
| `Truncated { offset }` | El búfer se acabó a mitad de un comando. |
| `UnknownOpcode { opcode, offset }` | |
| `UnknownKind { kind, offset }` | Un `CREATE_NODE` con un código mayor que 25. |
| `InvalidUtf8 { offset }` | |
| `Tree { opcode, offset, error }` | El núcleo rechazó la mutación: no existe ese nodo, id duplicado, etc. |

Todos llevan el **offset**, y `Tree` lleva además el **opcode**. Esa es la
diferencia entre «el búfer estaba mal» y «el `INSERT_CHILD` del byte 412 nombró
un padre que no existe», y es toda la razón por la que los errores tienen esta
forma.

:::caution[No hay rollback]
Un error deja el árbol con todo lo aplicado hasta ese punto. Es deliberado: un
búfer malformado es un bug del lado de JS, no una condición esperada, y dejarlo a
medias pone el fallo en la pantalla donde alguien lo va a ver en vez de
esconderlo detrás de una reversión limpia.
:::

## El codificador en Rust

`an-bridge` contiene también un codificador del mismo formato. El runtime no lo
usa: los búferes de verdad los escribe el preludio, en JavaScript.

Existe para que el decodificador se pueda probar sin arrancar un motor de JS, y
para que cualquier cambio del formato rompa los tests en **los dos** extremos a
la vez en lugar de dejar que los dos lados se separen hasta que algo se pinte mal
en un dispositivo.

## Dónde vive cada mitad

| | |
|---|---|
| Quien escribe | `packages/runtime/runtime.js` — el búfer de comandos del preludio |
| Quien lo llama | `packages/platform-native/src/renderer.ts` y `native-node.ts` |
| Quien lee | `crates/an-bridge/src/protocol.rs` |
| A qué se aplica | `crates/an-core/src/tree.rs` — el árbol en la sombra |

Un tick de Angular llena el búfer; `commit()` lo decodifica, resuelve el layout
sobre taffy, lo compara con el frame anterior y produce la lista de `MountOp` que
aplica el host. Ese camino entero está descrito en
[cómo funciona](/es/guide/how-it-works/).
