---
title: Navegación y el router
description: El router de Angular sin barra de direcciones — un historial en memoria, una pila nativa que mantiene viva la pantalla de atrás, y por dónde entran el gesto de volver y el botón de Android.
sidebar:
  order: 6
---

El router de Angular funciona aquí sin cambios: el mismo `provideRouter`, los
mismos parámetros de ruta, los mismos guards y resolvers, el mismo
`withComponentInputBinding`. Lo que cambia es lo de debajo. No hay barra de
direcciones ni historial del navegador, así que al router se le da una pila
propia, y la transición entre pantallas es una nativa de verdad y no un cambio
de DOM.

```ts
import { provideRouter, withComponentInputBinding } from '@angular/router'
import {
  bootstrapNativeApplication,
  NATIVE_LOCATION_PROVIDERS,
  NATIVE_STACK_PROVIDERS
} from '@angular-native/platform'

bootstrapNativeApplication(AppComponent, {
  providers: [
    ...NATIVE_LOCATION_PROVIDERS,
    ...NATIVE_STACK_PROVIDERS,
    provideRouter(
      [
        { path: '', component: HomePage },
        { path: 'ship/:id', component: DetailPage }
      ],
      withComponentInputBinding()
    )
  ]
})
```

Y en la plantilla raíz, donde iría un `<router-outlet>`:

```html
<an-native-stack />
```

## Los dos juegos de providers

Están separados porque contestan a preguntas distintas, y uno de los dos es
opcional.

**`NATIVE_LOCATION_PROVIDERS`** es lo que hace que el router funcione siquiera.
El router de Angular está escrito contra la History API del navegador; aquí
recibe `NativePlatformLocation`, un `PlatformLocation` sobre una pila en memoria.
Las rutas siguen siendo URLs —`/ship/42` es una URL y `:id` sigue enlazando—
simplemente no existen fuera de la app.

**`NATIVE_STACK_PROVIDERS`** es lo que hace que volver sea instantáneo. Instala
una `RouteReuseStrategy` que mantiene vivas las pantallas que quedan por debajo
de la cima de la pila.

Sin eso, el router hace lo que hace en la web: destruye el componente que dejas y
lo vuelve a construir desde cero cuando regresas. En un móvil ese es el valor por
defecto equivocado y se nota mucho: se pierde la posición del scroll, lo que
hubiera escrito en el formulario, y cualquier estado que tuviera el componente.
Es el mecanismo que ofrece el propio Angular exactamente para esto, y el mismo que
usa Ionic.

La estrategia además poda. Cuando una navegación va **hacia atrás**, lo guardado
para la pantalla que se abandona se tira: no se va a volver a llegar a ella por
ese camino, y guardar sin podar es una fuga con buen nombre.

## `<an-native-stack>`

Va donde iría un `<router-outlet>` y hace el mismo trabajo, con tres
diferencias:

- la pantalla que llega entra deslizándose y la que se deja sale por detrás,
- la que se deja sigue viva, así que volver es instantáneo,
- el gesto del borde de iOS y el botón de volver de Android navegan hacia atrás.

Por dentro hay un `an-stack-view` —la primitiva nativa— envolviendo un
`<router-outlet>` normal. El componente no tiene magia; lo que aporta es la
dirección de `[transition]` y el cableado del `(back)`.

## Hacia delante y hacia atrás

El router dice *adónde* has ido. No dice si eso fue hacia delante o hacia atrás,
y tanto la dirección de la animación como la decisión de conservar o tirar una
pantalla dependen de saberlo.

Lo que los distingue es la posición en la pila del historial.
`NativePlatformLocation` expone `historyIndex`, y `NavigationDirection` lo
compara entre eventos `NavigationEnd`:

```ts
readonly direction = inject(NavigationDirection).direction  // 'push' | 'pop' | 'none'
```

Esa señal es pública, así que un componente puede apoyarse en ella para una
transición propia.

:::note[Por qué la estrategia de reutilización lee el índice directamente]
`NativeStackReuseStrategy` no inyecta `NavigationDirection`, aunque ahí es donde
vive la dirección. `NavigationDirection` inyecta el `Router`, y el `Router`
inyecta la estrategia de reutilización — un ciclo. Leer el índice lo rompe, y el
índice es de donde sale la dirección de todas formas.
:::

## Por dónde entra el gesto de volver

Las dos plataformas desembocan en la misma llamada. El
`UIScreenEdgePanGestureRecognizer` del borde izquierdo en iOS y el botón de
volver de Android —el botón, el gesto, lo que tenga puesto el usuario— llegan al
output `(back)` de `an-stack-view`, y `<an-native-stack>` contesta con
`Location.back()`.

Desde el punto de vista del router no ha pasado nada especial: ha llegado un
popstate, exactamente como en un navegador. Por eso los guards siguen funcionando
con un gesto de volver del sistema en vez de ser esquivados por él.

`canGoBack` en `NativePlatformLocation` es lo que consulta el gesto antes de
decidir que tiene adónde ir. En el fondo de la pila no hay pantalla detrás, y el
gesto no hace nada.

## El historial sobrevive a una recarga en caliente

Guardar un fichero reconstruye el bundle y lo evalúa en un motor nuevo. Sin
ayuda eso te devolvería a la primera pantalla de la app cada vez, que es lo
primero que raspa en un ciclo de desarrollo.

La pila se guarda con `globalHotState` y se restaura al otro lado, así que un
guardado te deja en la pantalla en la que estabas, cuatro niveles adentro, con los
parámetros de ruta intactos. Esto no cuesta nada en una build de producción: no
hay recarga, así que el valor guardado no se lee nunca.

El mismo mecanismo está disponible para tus componentes — ver
[la recarga en caliente en la referencia del CLI](/es/reference/cli/#what-a-save-actually-does)
para la señal `hotState`.

## Lo que no hay

**No hay fragmentos `#`.** No hay barra de direcciones donde ponerlos, así que
`onHashChange` devuelve un desuscriptor que no hace nada y `hash` es lo que
llevara la propia cadena de la URL.

**No hay deep links todavía.** Una URL que llegue de fuera de la app —un esquema
propio, un universal link— no está conectada al router. La pila está en memoria y
empieza en `/`.

**`protocol` es `app:` y `hostname` es `localhost`.** Tienen que contestar algo,
porque `PlatformLocation` los declara, y no hay nada veraz que contestar. Nada
del router los lee.

## Las pestañas no son rutas

`an-tab-bar` es un control, no un outlet del router. Reporta `(select)` con un
índice y tú decides qué significa: cambiar una señal, o navegar. Que tenga una
pila de rutas por pestaña, como hace `UITabBarController` de forma nativa, no es
algo que el framework haga por ti.

## El ejemplo

```bash
cargo an dev examples/router
```

Dos pantallas, una ruta con parámetro, `withComponentInputBinding` y el gesto de
volver. `scripts/check-router.sh` lo mueve a través de `headless` y comprueba que
una navegación monta los nodos de la segunda pantalla y que volver trae los de la
primera de vuelta en lugar de reconstruirlos.
