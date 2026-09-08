---
title: Navegación y el router
description: El router de Angular sin barra de direcciones — un historial en memoria, una pila nativa que mantiene viva la pantalla de atrás, y por dónde entran el gesto de volver y el botón de Android.
sidebar:
  order: 7
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

`canGoBack` en `NativePlatformLocation` es lo que decide si el gesto es de la
app siquiera. `<an-native-stack>` se suscribe a `(back)` solo mientras sea
cierto, así que en el fondo de la pila no escucha nadie y la pulsación cae al
sistema: Android saca su callback del dispatcher, dibuja la previsualización de
vuelta al inicio y sale de la app; en tvOS el botón de menú llega a la
plataforma. Nada está atado, así que nada se traga.

## El historial sobrevive a una recarga en caliente

Guardar un fichero reconstruye el bundle y lo evalúa en un motor nuevo. Sin
ayuda eso te devolvería a la primera pantalla de la app cada vez, que es lo
primero que raspa en un ciclo de desarrollo.

La pila se guarda con `globalHotState` y se restaura al otro lado, así que un
guardado te deja en la pantalla en la que estabas, cuatro niveles adentro, con los
parámetros de ruta intactos. Esto no cuesta nada en una build de producción: no
hay recarga, así que el valor guardado no se lee nunca.

El mismo mecanismo está disponible para tus componentes — ver
[la recarga en caliente en la referencia del CLI](/es/reference/cli/#qué-hace-de-verdad-un-guardado)
para la señal `hotState`.

## Deep links

Una URL que llega de fuera de la app —otra app, una notificación, un enlace en un
navegador— se convierte en una ruta. No hay que proveer nada más allá de
`NATIVE_LOCATION_PROVIDERS`, que ya tienes.

`an add ios` y `an add android` declaran un esquema por ti, y el esquema es tu
identificador de bundle. No es una elección estética: un esquema de URL se
reclama en todo el dispositivo, así que dos apps que reclamen `myapp` dejan al
sistema decidiendo entre ellas, y el identificador es la única cadena que ya es
tuya y de nadie más. Así que:

```bash
# simulador de iOS
xcrun simctl openurl booted 'com.example.myapp://ship/2'

# Android
adb shell am start -a android.intent.action.VIEW -d 'com.example.myapp://ship/2'
```

abre la app en `/ship/2`, esté arrancando o ya en marcha.

### Cómo una URL se convierte en ruta

Un **esquema propio** no lleva sitio dentro. `myapp://ship/2` le parece a un
parser de URLs la ruta `/2` en el host `ship`, y nadie que escriba ese enlace
quiere decir eso, así que todo lo que va después del esquema es la ruta:
`/ship/2`.

Un **universal link o App Link** sí lleva un sitio, y el sitio no es parte de la
ruta: `https://example.com/ship/2` se convierte en `/ship/2`.

La query y el fragmento vienen con ella. Lo que no sea una URL en absoluto se
ignora en lugar de tratarse como `/`: navegar a la pantalla inicial porque un
enlace era ilegible es peor que no hacer nada.

### Arranque en frío y app ya en marcha

Son dos casos de verdad distintos y los dos funcionan.

En un **arranque en frío** el sistema lanza el proceso *por culpa* de la URL, y
esta llega al shell antes de que Angular exista. El shell se la entrega al núcleo
antes de evaluar el bundle; `NativePlatformLocation` la lee al construirse y la
pone en la pila como la entrada *actual*, no encima de `/`. Así la primera
pantalla que pinta la app ya es la correcta —no se ve pasar la pantalla inicial—
y el gesto de volver sale de la app, porque de verdad no hay nada detrás.

Con la app **ya en marcha**, la URL llega como evento y se apila. La pantalla en
la que estaba la persona sigue viva debajo, la transición anima hacia adelante y
volver la devuelve donde estaba. Un enlace a la ruta que la app ya está mostrando
no hace nada.

El relevo entre los dos casos es deliberado y no cuestión de suerte: coger la
cola de enlaces en espera es el mismo acto que decir «ya estoy escuchando», así
que una URL que caiga justo en el hueco no puede entregarse dos veces ni
perderse.

### Hacer otra cosa con ella

Si quieres interceptar un enlace en vez de dejar que navegue —comprobar un token,
mapear una URL antigua a una ruta nueva— suscríbete y hazlo tú:

```ts
import { onDeepLink, routeFromUrl } from '@angular-native/platform'

const stop = onDeepLink((url) => {
  console.log('abierta con', url, '->', routeFromUrl(url))
})
```

`onDeepLink` ve los enlaces que llegan con la app en marcha. `routeFromUrl` es la
misma función que usa la location, exportada para que tu mapeo y el de la casa no
puedan discrepar.

### Lo que sigues teniendo que escribir a mano

El esquema propio está cableado. Un **universal link** o un **App Link** —una
dirección `https://` de verdad que abre tu app en lugar del navegador— no puede
estarlo, porque la mitad vive en tu servidor web y nada de un CLI puede ponerlo
ahí.

En **iOS** necesitas dos cosas más de las que escribe `an add ios`:

1. El entitlement `com.apple.developer.associated-domains`, con
   `applinks:example.com`. `an` no escribe tu fichero de entitlements, así que
   este es tuyo.
2. `https://example.com/.well-known/apple-app-site-association`, servido como
   `application/json` y sin redirección, nombrando tu Team ID y tu identificador
   de bundle. Apple lo descarga; si no puede, el enlace abre Safari en silencio,
   que es la razón más común de que un universal link «no funcione».

El shell ya atiende `continue userActivity`, así que con esas dos cosas en su
sitio no cambia nada más.

En **Android** añades un segundo `<intent-filter>` a
`android/AndroidManifest.xml`, al lado del que escribió `an add android`:

```xml
<intent-filter android:autoVerify="true">
    <action android:name="android.intent.action.VIEW" />
    <category android:name="android.intent.category.DEFAULT" />
    <category android:name="android.intent.category.BROWSABLE" />
    <data android:scheme="https" android:host="example.com" />
</intent-filter>
```

y sirves `https://example.com/.well-known/assetlinks.json` con tu nombre de
paquete y la huella SHA-256 del certificado de firma. `autoVerify` es lo que hace
que Android abra tu app sin preguntar; sin el fichero se cae a un selector.

:::note[Solo iOS y Android reciben una URL]
macOS, tvOS, visionOS, watchOS y Wear OS todavía no entregan deep links. La parte
del núcleo es común —el buzón y el lado de JS están en `an-bridge`, no en un
host—, así que lo que falta en esas plataformas es solo que el shell entregue la
URL: `application(_:open:urls:)` en AppKit, y el equivalente en el resto.
:::

## Lo que no hay

**No hay fragmentos `#`.** No hay barra de direcciones donde ponerlos, así que
`onHashChange` devuelve un desuscriptor que no hace nada y `hash` es lo que
llevara la propia cadena de la URL.

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

El mismo ejemplo es lo que demuestra los deep links, sin simulador:

```bash
# arranque en frío: la URL entra antes de evaluar el bundle
AN_OPEN_URL='playground://ship/3' \
  cargo run -p an-bridge --example headless -- build/bundle/router/main.js 3

# ya en marcha: llega a mitad de camino
AN_OPEN_URL_LATER='playground://ship/2' \
  cargo run -p an-bridge --example headless -- build/bundle/router/main.js 6
```

`scripts/check-deep-links.sh` corre los dos y comprueba que el primero pinta la
pantalla de detalle sin que la lista llegue a aparecer, y que el segundo apila.
