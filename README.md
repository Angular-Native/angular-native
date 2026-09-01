# angular-native

React Native, pero para Angular, y con el núcleo en Rust.

Angular zoneless con signals corriendo en un motor JS embebido; el árbol de UI,
el layout y el montaje sobre **vistas nativas reales** los lleva Rust. Sin DOM,
sin WebView, sin zone.js. Un solo núcleo para iOS y Android.

```text
  hilo del motor                                        hilo de UI
  ─────────────────────────────────────────────         ──────────────────────
  Angular (JS)                                          CADisplayLink
      │ Renderer2                                       Choreographer
      ▼                                                       │ pide frame
  __an_dom ──▶ búfer binario ──▶ ShadowTree                   ▼
                                     │ commit           MountSide
                               taffy (layout)                 │
                                     │ diff                   ▼
                               Frame { MountOp[] } ──────▶ HostRenderer
                                                        (UIKit / android.view)
```

Un componente Angular normal, sin nada especial salvo que los elementos son
primitivas nativas:

```ts
@Component({
  selector: 'app-root',
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-view [style.gap]="'16'" [backgroundColor]="'#0b1020'">
      <an-text [fontSize]="28" [color]="'#f4f7ff'">angular-native</an-text>
      <an-switch [on]="alertas()" (onChange)="alertas.set($event)" />
      <an-view
        [backgroundColor]="'#1e2a4a'"
        [animate]="200"
        [translateX]="x()"
        (pan)="onPan($event)"></an-view>
      @if (seconds() >= 3) {
        <an-text [fontSize]="14">han pasado {{ seconds() }} segundos</an-text>
      }
    </an-view>
  `
})
export class AppComponent {
  readonly seconds = signal(0)
  readonly alertas = signal(true)
  readonly x = signal(0)
  onPan(event: NativePanEvent) { this.x.set(event.translationX) }
}
```

```ts
bootstrapNativeApplication(AppComponent)
```

`<an-view>` acaba siendo una `UIView` en iOS y una `AnViewGroup` en Android; el
`<an-switch>`, un `UISwitch` y un `android.widget.Switch`.

## Empezar

```bash
cargo an dev                  # compila, lanza en el simulador y recarga al guardar
cargo an dev --android        # lo mismo, en el emulador de Android
cargo an ios                  # una sola vez, sin vigilar
cargo an android              # APK, emulador y lanzamiento
cargo an watchos              # reloj: .app de watchOS y simulador
cargo an build --release      # solo el bundle: 276 KB frente a 1,3 MB en debug
```

El reloj pide nightly: `aarch64-apple-watchos-sim` es un target de nivel 3 y su
`std` se construye en el momento. Ver [docs/watchos.md](docs/watchos.md).

Todo va por el mismo binario, `an`. No hay `.xcodeproj` ni Gradle: las
herramientas de cada SDK ya hacen el trabajo y el proceso cabe en un fichero
que se puede leer entero.

## Qué hay

**Primitivas** — `an-view`, `an-text`, `an-image`, `an-scroll-view`,
`an-text-input`, `an-textarea`, `an-stack-view`, `an-web-view`, `an-modal`,
`an-alert`.

**Controles del sistema** — `an-tab-bar`, `an-switch`, `an-slider`,
`an-activity-indicator`, `an-progress-bar`, `an-button`,
`an-segmented-control`, `an-stepper`, `an-search-bar`, `an-select`,
`an-date-picker`, `an-navigation-bar`, `an-icon`. No los dibuja el framework:
en iOS son `UITabBar`, `UISwitch`, `UISlider`, `UISegmentedControl`,
`UIStepper`, `UISearchBar`, `UIDatePicker`, `UINavigationBar` y compañía, con
el aspecto que tengan en esa versión del sistema. Los tres que Android no trae en la
plataforma —barra de pestañas, control segmentado y control de pasos— se
dibujan con vistas del sistema, y se dice cuál es cuál.

**Iconos** — SF Symbols en iOS y Material Symbols en Android, por nombre. No se
empaqueta ningún juego en iOS; en Android sí, porque el que trae la plataforma
lleva congelado desde 2011 y no es el de Material 3.

**Compuestos** — `an-virtual-list` (lista con reciclado de vistas),
`an-safe-area`, `an-native-stack` (navegación con pila y gesto de atrás).

**Gestos** — `press`, `doublePress`, `longPress`, `pan`, `pinch`, `rotation`,
`swipeLeft`/`Right`/`Up`/`Down`. Los reconocedores son los del sistema, así que
los umbrales de cuándo un gesto cuenta son los de cada plataforma.

**Transformaciones y animación** — `translateX`, `translateY`, `scale`,
`rotate`, y `[animate]="ms"` para que los cambios de esa vista dejen de ser un
salto. Las hace la plataforma en su hilo de dibujo, sin volver a pasar por
JavaScript en cada frame.

**Otros eventos** — `layout`, `safeArea`, `scroll`, `refresh`, `change`,
`focus`, `blur`, `submit`, `select`, `load`, `back`, `dismiss`.

**Angular** — plantillas AOT, señales, `@if`, `@for`, router con parámetros,
módulos nativos tipados, y refresco en caliente: al guardar cambia el código de
los componentes sin tirar la app, así que sigues en la misma pantalla y con lo
que llevaras escrito.

## Verificación sin dispositivo

```bash
./scripts/check-all.sh        # todo: tests, las ocho apps y las dos compilaciones cruzadas

cargo test                    # solo el núcleo Rust
./scripts/check-angular.sh    # la cadena entera: ngc, esbuild, QuickJS, taffy
./scripts/check-list.sh       # an-scroll-view, an-text-input, reciclado, módulo nativo
./scripts/check-router.sh     # navegación, parámetros y vuelta atrás
./scripts/check-controls.sh   # los controles del sistema y sus tamaños
./scripts/check-gestures.sh   # gestos, transformaciones y animación
./scripts/check-pickers.sh    # segmentos, desplegable, pasos, búsqueda y fecha
./scripts/check-web.sh        # cabecera, texto multilínea, navegador, hoja
./scripts/check-styles.sh     # que las dos listas de nombres de estilo no se separen
./scripts/check-kinds.sh      # que la etiqueta, la primitiva y el código digan lo mismo
./scripts/check-watchos.sh    # el modelo del reloj y su compilación cruzada
cargo run -p an-bridge --example headless -- build/bundle/hello-angular/main.js 6
```

`headless` monta el pipeline entero salvo la plataforma: evalúa un bundle,
avanza frames con un reloj falso, simula un toque, un arrastre, un
desplazamiento y una vuelta atrás, e imprime el árbol resuelto. Es la forma
rápida de depurar sin simulador, y es lo que usan todos los scripts.

## Crates

| Crate | Qué hace |
|---|---|
| `an-layout` | Props de estilo a `taffy::Style`, árbol de layout, medición de hojas |
| `an-core` | Shadow tree, mutaciones, commit, diff hacia `MountOp` |
| `an-host` | Traits `HostRenderer` y `TextMeasurer`, y las dos mitades del renderer |
| `an-bridge` | Motor JS (QuickJS), protocolo binario, módulos nativos, hilo del motor |
| `an-ios` | Host UIKit, medición, controles, animaciones y superficie C |
| `an-android` | Host JNI, medición con `StaticLayout` y puntos de entrada JNI |
| `an-watch` | Host watchOS: el árbol reflejado en un modelo que pinta SwiftUI |
| `an-cli` | La herramienta `an`: build, ios, android, watchos y servidor de desarrollo |

| Paquete npm | Qué hace |
|---|---|
| `packages/runtime` | Prelude JS: consola, temporizadores, `AbortController`, búfer de comandos |
| `packages/platform-native` | `Renderer2`, plataforma, `PlatformLocation`, navegación, módulos |
| `packages/primitives` | Todas las primitivas, controles y compuestos |

## Decisiones

- **Vistas nativas, no pintado propio.** Accesibilidad, IME, scroll y look del
  sistema salen gratis; el coste es una capa por plataforma para cada primitiva.
- **Los ids de nodo los asigna JS.** Crear un nodo no necesita viaje de vuelta
  al core, igual que los tags de Fabric.
- **Layout en `taffy`**, no Yoga: es Rust, y trae flexbox, grid y block.
- **Zoneless obligatorio.** Sin zone.js no hay que parchear temporizadores ni
  XHR dentro del motor JS, que es el 80% del dolor de NativeScript.
- **Un commit por frame.** El layout solo corre en `commit()`, y el `Frame`
  resultante lleva únicamente lo que cambió.
- **Búfer binario, no llamadas sueltas.** Un `@for` de 200 filas son ~1.200
  mutaciones. Con llamadas por mutación son 1.200 cruces de frontera; así es uno.
- **El motor JS vive en su propio hilo, y el de UI lo espera con plazo.** Si el
  turno de JS cabe en lo que queda de frame se monta en el mismo frame; si se
  pasa, el hilo de UI sigue y lo monta cuando llegue. El hilo aparte no es por
  paralelismo: QuickJS necesita unos 4 MB de pila para que el router de Angular
  complete una navegación, y el hilo principal de iOS tiene 1 MB que no se
  pueden cambiar.
- **JS no tiene reloj propio: tiene un turno por frame.** Los temporizadores
  avanzan con el vsync, así que el tiempo de la app es determinista y un test
  puede simular diez segundos sin esperarlos.
- **AOT siempre, nunca JIT.** `ngc` compila las plantillas en el build y el
  Angular Linker resuelve los paquetes publicados en modo parcial; el
  dispositivo no lleva `@angular/compiler`.
- **Primitivas como directivas tipadas, no `CUSTOM_ELEMENTS_SCHEMA`.** El
  esquema laxo exige un guion en el nombre y, peor, apaga la comprobación de
  propiedades: `[bakcgroundColor]` con errata pasaría el compilador y fallaría
  en silencio en el dispositivo.
- **Prefijo `an-` en todas las etiquetas**, como Ionic. Sin él, Angular se
  niega a auto-cerrar una etiqueta que se llame como un elemento de HTML, y eso
  costó dos nombres: el desplegable acabó llamándose `Picker` y el campo de
  varias líneas, `TextEditor`. Con prefijo vuelven a ser `<an-select>` y
  `<an-textarea>`, y `<an-view />` se cierra sola como cualquier otra.
- **De la etiqueta al núcleo, con una regla y no con una tabla.** Se le quita
  `an-` y se junta en PascalCase: `an-text-input` es `TextInput`. Añadir una
  primitiva no obliga a apuntarla en una lista de traducción que se pueda
  quedar atrás. El vocabulario del núcleo no cambia con la etiqueta: `an-select`
  sigue viajando como el `Picker` que el enum de Rust y los tres hosts conocen.
- **Cuánto mide un control lo decide la plataforma.** Un `UISwitch` no mide lo
  mismo en iOS 17 que en iOS 26, ni con texto grande de accesibilidad. Se les
  pregunta al arrancar, en el hilo principal.
- **Los eventos se registran solo si la plantilla los pide.** `(press)` es una
  salida sobre un observable frío: el reconocedor se engancha al suscribirse.
- **`onLayout` lo emite el core**, no la plataforma: es él quien calcula el
  marco, así que funciona igual en iOS y en Android sin implementarlo dos veces.
- **El host de un componente no es una vista.** Si nadie le pone estilo, prop ni
  oyente, no llega a crearse y sus hijos cuelgan del abuelo. Fabric lo llama
  *view flattening*; aquí se decide en el lado JS, que ve la secuencia entera
  antes de mandarla.
- **Reciclar, no rehacer.** `an-virtual-list` monta un número fijo de ranuras y al
  desplazarse no crea ni destruye ninguna: cambia lo que enseña cada una.
- **Nada de `@angular/platform-browser`.** Arrastra `DomAdapter`,
  `DomRendererFactory2` y el sanitizador de HTML, todo asumiendo que existe un
  DOM. La plataforma propia son ~70 líneas.

## Lo que no está hecho

- **El de pasos de Android no es un control, es un montaje.** Material 3 no
  define ninguno, así que se arma con dos botones de icono y un rótulo suyos.
  El resto de controles sí son componentes de la librería.
- **El refresco en caliente no llega al framework.** Cambiar un componente de la
  app conserva el estado; cambiar `packages/` o una dependencia obliga a
  reiniciar, porque en el intérprete solo cabe una copia de Angular. Se avisa y
  se reinicia, no se enseña código viejo.
- **El reloj va por la mitad.** watchOS pinta `an-view`, `an-text`, `an-button` y
  `an-scroll-view`, que es lo que da para una pantalla de verdad, pero le faltan el
  resto de primitivas, los gestos más allá del toque, la animación y la recarga
  en caliente. No es un port del host de iOS: watchOS no tiene jerarquía de
  `UIView`, así que el árbol se refleja en un modelo que redibuja SwiftUI. El
  porqué y lo que falta, en [docs/watchos.md](docs/watchos.md).

## Desarrollo

`NaiveMeasurer` aproxima la medición de texto para que el núcleo sea testeable
sin plataforma. En el dispositivo mandan `UikitMeasurer` y `JniMeasurer`, que
cachean por (texto, fuente, ancho disponible): el layout mide cada nodo varias
veces por frame y sin caché eso son cientos de cruces de frontera.

Las herramientas se descubren solas: el SDK de Android por `ANDROID_HOME` o la
ruta estándar, y dentro de él la última versión de build-tools y de plataforma.
El NDK y los flags de bindgen están fijados en `.cargo/config.toml`.
