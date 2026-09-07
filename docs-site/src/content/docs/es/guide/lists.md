---
title: Listas largas
description: "`an-virtual-list` monta un número fijo de vistas y no crea ninguna más mientras haces scroll — cómo funciona el reciclaje, por qué hay que declarar la altura de fila, y cuándo un `@for` normal es mejor respuesta."
sidebar:
  order: 7
---

Un `@for` sobre diez mil filas crea diez mil vistas nativas. En un móvil eso no
es lento, es fatal: la memoria son `UIView` y `ViewGroup` de verdad, y el commit
que las monta es un frame lo bastante largo como para ser un congelón.

`an-virtual-list` monta tantas filas como caben en pantalla más un margen, y
hacer scroll no crea ni destruye **ninguna**. Cambia lo que enseña cada una y
dónde se coloca. Diez mil filas cuestan las mismas veinte vistas nativas que
veinte filas.

```html
<an-virtual-list [items]="rows()" [itemHeight]="64" [style.flexGrow]="'1'">
  <ng-template let-row let-i="index">
    <an-view [style.padding]="'16'" [style.gap]="'4'">
      <an-text [fontSize]="16">{{ row.name }}</an-text>
      <an-text [fontSize]="13" [color]="'#8a93a6'">fila {{ i }}</an-text>
    </an-view>
  </ng-template>
</an-virtual-list>
```

```ts
import { VirtualList } from '@angular-native/primitives'
```

## Cómo funciona el reciclaje

La lista mantiene un carrusel fijo de huecos. Cada hueco tiene una `key` que es
**su posición en el carrusel**, no el elemento que le toque enseñar, y la
plantilla se renderiza con `track slot.key`.

Ese detalle es todo el mecanismo. Como la clave no cambia al hacer scroll,
Angular no destruye y reconstruye la vista embebida: le actualiza los bindings.
`NgTemplateOutlet` se comporta igual mientras las claves del objeto de contexto
no cambien, que es el caso aquí: cada contexto es `{ $implicit, index }`.

Así que un scroll de doscientas filas produce doscientas *actualizaciones de
prop* en vez de doscientos pares de crear-y-destruir. En el cable eso es la
diferencia entre un puñado de opcodes `SET_PROP` y unos miles de
`CREATE_NODE`/`DESTROY_NODE`.

Por dentro, las filas van posicionadas en absoluto dentro de una vista
espaciadora muy alta, así que el tamaño de contenido de la vista con scroll es la
altura completa de la lista desde el primer frame: la barra dice la verdad y el
scroll no crece bajo tu dedo.

## Hay que declarar la altura de fila

No hay forma de saber qué hay en el desplazamiento 12.000 sin haber medido todo
lo que va por encima. Así que la altura es una prop, no una medición:

```html
<an-virtual-list [items]="rows()" [itemHeight]="64">
```

Con un número para todas las filas no hay nada que guardar: la fila `i` empieza
en `i * altura`, y la fila que está en un desplazamiento dado sale de una
división.

Para filas distintas, `itemHeight` admite una función:

```ts
protected readonly rowHeight = (row: Row, index: number) => (row.expanded ? 128 : 64)
```

```html
<an-virtual-list [items]="rows()" [itemHeight]="rowHeight">
```

La función se llama **una vez por fila cada vez que la lista cambia**, no en cada
scroll. Los desplazamientos se acumulan en un array de sumas parciales y después
se buscan por bisección, que sobre cinco mil filas son trece comparaciones.

:::caution[Una altura que miente es un hueco]
Después nadie mide la fila para comprobarlo. Si la plantilla renderiza más alto
de lo que dice el número, las filas se solapan; más bajo, y queda una franja de
fondo entre ellas. La altura es un contrato.
:::

## Las props

| Prop | Tipo | Por defecto | Qué hace |
|---|---|---|---|
| `items` | `readonly T[]` | obligatoria | |
| `itemHeight` | `number` \| `(item, index) => number` | obligatoria | |
| `overscan` | `number` | `4` | Huecos de repuesto en cada extremo, para que un scroll rápido no deje claros. |
| `refreshing` | `boolean` | `false` | Si el indicador de recarga está abierto. |

| Evento | Carga | |
|---|---|---|
| `refresh` | — | Tirar para recargar. Si nadie escucha, el gesto no existe. |

Tirar para recargar es de ida y de vuelta por separado: el gesto abre el
indicador y emite `refresh`, y poner `[refreshing]` de nuevo a `false` es lo que
lo cierra. La lista no decide cuándo han llegado tus datos.

```html
<an-virtual-list
  [items]="rows()"
  [itemHeight]="64"
  [refreshing]="loading()"
  (refresh)="reload()"
  [style.flexGrow]="'1'">
```

## Hay que darle sitio

Tanto la lista como la vista con scroll de dentro tienen que tener permiso para
ser más pequeñas que su contenido, o abren el layout y no queda nada por lo que
hacer scroll. El componente pone `minHeight: 0`, `flexBasis: 0`, `flexShrink: 1`
y `overflow: hidden` en su propio host, pero el **padre** sigue teniendo que
darle una parte del espacio:

```html
<an-view [style.flexGrow]="'1'">
  <an-virtual-list [items]="rows()" [itemHeight]="64" [style.flexGrow]="'1'" />
</an-view>
```

Una lista sin `flexGrow` en una columna que tiene más hijos se medirá a la altura
de su contenido, que son las diez mil filas enteras, y entonces no hay viewport
contra el que reciclar.

## Cuándo no usarla

Un `@for` normal dentro de un `an-scroll-view` es la respuesta correcta más a
menudo de lo que la gente espera. Cuesta una vista nativa por fila, lo cual va
bien hasta unos cientos, y a cambio:

- las filas pueden tener cualquier altura, medida en vez de declarada,
- no hay ningún carrusel sobre el que razonar,
- una fila conserva su propio estado entre scrolls, porque no se recicla nunca.

La regla práctica: si el número de filas lo acota algo que ha escrito o elegido
una persona, usa `@for`. Si lo acota lo que tenga un servidor, usa la lista
virtual.

## Cuidado con lo que captura la plantilla

La plantilla de una fila reciclada se vuelve a evaluar con un contexto nuevo, así
que todo lo que se derive de la fila hay que derivarlo **en la plantilla o del
contexto**, no guardarlo en un campo de un componente hijo. Un componente hijo
dentro de la fila conserva su instancia entre reciclajes, que es justo el
objetivo — así que un componente que cargue algo en su constructor a partir de la
fila que vio primero seguirá enseñando los datos de aquella primera fila.

Dale a esa fila un input de verdad y deriva de él, y el reciclaje es invisible.

## El ejemplo

```bash
cargo an dev examples/kitchen
```

`examples/kitchen` lleva una lista de cinco mil filas con dos alturas mezcladas.
`scripts/check-list.sh` la mueve a través de `headless` y comprueba los dos
números que importan:

```text
ok   scrolling recycles: not one new view
ok   69 native views for 5000 rows
```

Una regresión que reintroduzca la creación por fila falla ahí, en un segundo, y
no en un dispositivo con una lista larga.
