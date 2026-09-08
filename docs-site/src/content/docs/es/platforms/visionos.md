---
title: visionOS
description: Angular en el Vision Pro — el mismo núcleo, el mismo host que iOS, y una ventana que no es una pantalla.
sidebar:
  order: 5
---

Angular en el Vision Pro: mismo núcleo, mismo layout, mismo bundle y el mismo
host. visionOS trae UIKit con jerarquía de `UIView` y marcos absolutos, igual
que iOS y que tvOS, así que `an-ios` compila para el visor sin reescribir nada.

```bash
cargo an visionos                  # examples/hello-vision en el simulador
./scripts/check-visionos.sh
```

Lo que cambia de verdad no es el catálogo de controles —está casi entero—, es
**de dónde sale la ventana y qué hay detrás de ella**.

## La tercera familia

Es la tercera rama del mismo enum `Family` de
`crates/an-cli/src/ios.rs`, y hereda todo lo que
ese módulo ya sabía: el `Info.plist` que puede aportar un proyecto de fuera, la
comprobación de que ese plist dice lo mismo que el proyecto, y `build_dir()`.

| | visionOS |
|---|---|
| Target de Rust | `aarch64-apple-visionos-sim` (**nivel 3**) |
| Triple de swiftc | `arm64-apple-xros1.0-simulator` |
| SDK de `xcrun` | `xrsimulator` |
| Runtime en `simctl` | aparece como `xrOS` |
| `Info.plist` | `shells/visionos/Resources` |
| Sufijo del `.app` | `Vision` / `.vision` |
| `std` de Rust | se construye con `-Z build-std` |

El nombre comercial cambió y el del compilador no: en el triple de `swiftc`, en
la variable de despliegue (`XROS_DEPLOYMENT_TARGET`) y en el nombre del runtime
de `simctl`, visionOS sigue siendo **xrOS**. Buscar «visionOS» en la lista de
`simctl` no encuentra nada.

## La ventana no es una pantalla

Esta es la diferencia que toca al modelo de marcos absolutos, y conviene
mirarla de frente antes de dar por bueno que «se ve igual que iOS».

**No hay `UIScreen`.** Está marcado `API_UNAVAILABLE(visionos)`, así que
`UIScreen.main.bounds` no compila siquiera. Una app no ocupa una pantalla:
ocupa una ventana que el usuario cuelga en la habitación, mueve y redimensiona
tirando de la esquina cuando quiere. De esa ventana solo se sabe a través de su
`UIWindowScene`, y el sistema solo crea uno si el manifiesto declara quién lo
atiende:

```text
  Info.plist                            shells/ios/Sources/SceneDelegate.swift
  UIApplicationSceneManifest  ──────▶   @objc(SceneDelegate)
    UISceneDelegateClassName              scene(_:willConnectTo:options:)
    = "SceneDelegate"                       └─▶ UIWindow(windowScene:)
```

Ese nombre es el de Objective-C, el que le pone `@objc` a la clase de Swift. Si
los dos dejaran de coincidir, la escena se conectaría, nadie crearía la ventana
y **la app arrancaría en negro sin un solo error**: por eso
`check-visionos.sh` compara las dos cadenas.

**Al modelo de marcos absolutos no le pasa nada.** El core nunca preguntó por
una pantalla: recibe un viewport y coloca. Lo que cambia es que ese viewport ya
no es una constante durante toda la vida de la app. El shell pide un tamaño al
abrir —`requestGeometryUpdate(.Vision(size:))`, que es una *preferencia*: el
sistema puede dársela o no— y el tamaño de verdad llega por
`viewDidLayoutSubviews`, igual que en las otras dos familias. Lo que sí cambia
es lo que puede escribir una plantilla: **nada en puntos fijos**. Un ancho de
`390` es una suposición sobre una pantalla que aquí no existe;
`examples/hello-vision` está escrito con porcentajes y `flexGrow`, y eso es lo
que comprueba el script.

**`deviceInfo.scale` sale 0.** No hay escala de pantalla que preguntar: la app
se dibuja para dos ojos y a la distancia a la que el usuario haya puesto la
ventana. Se devuelve 0 y se dice aquí, en vez de inventarse un `2.0` que
alguien acabaría usando para calcular píxeles.

## El fondo es cristal, y taparlo se nota

La ventana de visionOS ya trae fondo: el cristal que dibuja el sistema, con su
desenfoque y su sombra sobre la habitación de verdad. El shell deja la raíz
transparente (`view.backgroundColor = .clear`) en vez del negro que pone en las
otras dos familias.

Un `[backgroundColor]` en el contenedor de arriba lo tapa entero, y lo que
queda es una losa opaca flotando en el salón. No es un error —la app se ve— y
por eso está comprobado: `check-visionos.sh` mira que el contenedor raíz de
`hello-vision` siga sin pintar nada.

## Apuntar con la mirada

visionOS **sí tiene toques**: el pellizco con la mano cuenta como un toque
indirecto y llega por los mismos reconocedores que en iOS, así que aquí no hace
falta ningún motor de foco como el de la tele. Lo que cambia es cómo sabe el
usuario qué va a pulsar: apunta con la mirada, y sin realce no hay forma de ver
dónde está apuntando.

Ese realce lo dibuja el sistema, fuera del proceso de la app, pero solo si la
vista lo pide con `hoverStyle`. Una `UIView` con un reconocedor de toque no lo
pide: el valor de fábrica es `nil`. Por eso el host le pone `automaticStyle` a
toda vista que se hace pulsable (`crates/an-ios/src/hover.rs`).
`automaticStyle` es el realce del sistema con la forma que UIKit deduce de la
vista: no se dibuja nada a mano, y si visionOS cambia su aspecto en una
versión, esto cambia con él.

Y lo que no hay:

| Evento | Qué pasa en visionOS |
|---|---|
| `(back)` | No hay ningún gesto del sistema para volver: `UIScreenEdgePanGestureRecognizer` no está en el SDK y la ventana no tiene bordes que arrastrar. Se cierra por su barra, y dentro de la app el camino de vuelta tiene que ser un botón de la plantilla. Se dice en el log. |

## Qué está visto y qué no

Esto es importante y va aparte, porque no todo lo de arriba tiene el mismo
respaldo.

**Visto corriendo en el simulador de visionOS 26.5:** la app arranca, la
ventana nace de la escena, el árbol se monta con los marcos que calculó taffy,
el texto se mide con el `UIFont` de verdad, el cristal del sistema se ve por
detrás y por entre las tarjetas, y una pulsación llegó hasta la señal del
componente y cambió el rótulo.

**No verificado:** no he conseguido *provocar* esa pulsación de forma
repetible desde fuera. `xcrun simctl` no tiene ningún verbo para el puntero, y
un clic sintético sobre la ventana del simulador —con `cliclick`, con las
coordenadas de la ventana bien calculadas— no llega a la app: el simulador de
visionOS usa el ratón para mirar alrededor y para pellizcar, y esa entrada no
se deja conducir desde la línea de órdenes como sí se deja la del mando de la
tele. Lo que hay, por tanto, es una captura con el resultado de una pulsación y
ninguna forma de repetirla en un script. Mientras eso siga así, aquí no hay
equivalente de `scripts/tv-remote.sh`, y `check-visionos.sh` comprueba la
pulsación por el camino de siempre: el renderer sin pantalla.

**Tampoco verificado:** el realce de la mirada. El host le pone `hoverStyle` a
las vistas pulsables, pero sin poder mover la mirada desde fuera no he podido
ver el realce puesto en una captura.

## Lo que falta

- **Icono.** El `Info.plist` no lleva `CFBundleIcons`: el icono de visionOS es
  en capas y vive en un catálogo de assets compilado con `actool`. La app se
  instala y se lanza; en la parrilla sale sin icono.
- **Los plugins se compilan con sus fuentes de iOS**, que es lo único que
  declaran: el manifiesto `angularNative` tiene sección `ios` y no `visionos`, y
  es el mismo shell y el mismo protocolo `AnPlugin`. Si alguno usa API que el
  visor no tiene, el enlazado se para con el error de swiftc; `an visionos` lo
  avisa antes de compilar para que no llegue de sorpresa.
- **Cuatro salidas que llegan a la suscripción y no al visor**: `(hover)`,
  `(crown)` y `(crownIdle)`, que no entrega ninguna familia de UIKit —la mirada
  no es un puntero que una app pueda leer, y no hay rueda— y `(back)`, porque
  `UIScreenEdgePanGestureRecognizer` no está en el SDK y esta ventana no tiene
  borde del que tirar, así que la vuelta atrás tiene que ser un botón de la
  plantilla. Cada rechazo se dice una vez, al suscribirse la plantilla.
- **Nada volumétrico.** Esto es una ventana plana en un espacio 3D, que es lo
  que visionOS llama una *window*. Ni `volume` ni espacio inmersivo: las dos
  cosas son SwiftUI y RealityKit, y no hay `UIView` que montar en ellas, así
  que no serían este host sino otro, como pasó con el reloj.
- **Dispositivo de verdad.** Todo esto está visto en el simulador. Un Vision
  Pro físico pediría firma, y la mirada de verdad no es un ratón.
