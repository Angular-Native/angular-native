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
    <View [style.gap]="'16'" [backgroundColor]="'#0b1020'">
      <Text [fontSize]="28" [color]="'#f4f7ff'">angular-native</Text>
      <Switch [on]="alertas()" (onChange)="alertas.set($event)" />
      <View
        [backgroundColor]="'#1e2a4a'"
        [animate]="200"
        [translateX]="x()"
        (pan)="onPan($event)"></View>
      @if (seconds() >= 3) {
        <Text [fontSize]="14">han pasado {{ seconds() }} segundos</Text>
      }
    </View>
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

`<View>` acaba siendo una `UIView` en iOS y una `AnViewGroup` en Android; el
`<Switch>`, un `UISwitch` y un `android.widget.Switch`.

## Empezar

```bash
cargo an dev                  # compila, lanza en el simulador y recarga al guardar
cargo an dev --android        # lo mismo, en el emulador de Android
cargo an ios                  # una sola vez, sin vigilar
cargo an android              # APK, emulador y lanzamiento
cargo an build --release      # solo el bundle: 276 KB frente a 1,3 MB en debug
```

Todo va por el mismo binario, `an`. No hay `.xcodeproj` ni Gradle: las
herramientas de cada SDK ya hacen el trabajo y el proceso cabe en un fichero
que se puede leer entero.

## Qué hay

**Primitivas** — `View`, `Text`, `Image`, `ScrollView`, `TextInput`,
`TextEditor`, `StackView`, `WebView`, `Modal`, `Alert`.

**Controles del sistema** — `TabBar`, `Switch`, `Slider`, `ActivityIndicator`,
`ProgressBar`, `Button`, `SegmentedControl`, `Stepper`, `SearchBar`, `Picker`,
`DatePicker`, `NavigationBar`, `Icon`. No los dibuja el framework: en iOS son
`UITabBar`, `UISwitch`, `UISlider`, `UISegmentedControl`, `UIStepper`,
`UISearchBar`, `UIDatePicker`, `UINavigationBar` y compañía, con el aspecto que
tengan en esa versión del sistema. Los tres que Android no trae en la
plataforma —barra de pestañas, control segmentado y control de pasos— se
dibujan con vistas del sistema, y se dice cuál es cuál.

**Iconos** — SF Symbols en iOS y Material Symbols en Android, por nombre. No se
empaqueta ningún juego en iOS; en Android sí, porque el que trae la plataforma
lleva congelado desde 2011 y no es el de Material 3.

**Compuestos** — `VirtualList` (lista con reciclado de vistas), `SafeArea`,
`NativeStack` (navegación con pila y gesto de atrás).

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
módulos nativos tipados, y recarga en caliente que conserva el estado.

## Verificación sin dispositivo

```bash
./scripts/check-all.sh        # todo: tests, las siete apps y las dos compilaciones cruzadas

cargo test                    # solo el núcleo Rust
./scripts/check-angular.sh    # la cadena entera: ngc, esbuild, QuickJS, taffy
./scripts/check-list.sh       # ScrollView, TextInput, reciclado, módulo nativo
./scripts/check-router.sh     # navegación, parámetros y vuelta atrás
./scripts/check-controls.sh   # los controles del sistema y sus tamaños
./scripts/check-gestures.sh   # gestos, transformaciones y animación
./scripts/check-pickers.sh    # segmentos, desplegable, pasos, búsqueda y fecha
./scripts/check-web.sh        # cabecera, texto multilínea, navegador, hoja
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
| `an-cli` | La herramienta `an`: build, ios, android y servidor de desarrollo |

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
- **Reciclar, no rehacer.** `VirtualList` monta un número fijo de ranuras y al
  desplazarse no crea ni destruye ninguna: cambia lo que enseña cada una.
- **Nada de `@angular/platform-browser`.** Arrastra `DomAdapter`,
  `DomRendererFactory2` y el sanitizador de HTML, todo asumiendo que existe un
  DOM. La plataforma propia son ~70 líneas.

## Lo que no está hecho

- **Un nombre de estilo que nadie reconoce no avisa.** `[style.loQueSea]` cae
  como prop al host, el host no la usa, y no pasa nada. Ha mordido cuatro veces
  en un día: `[style.fontSize]` llegaba con guion, el hueco del área segura se
  quedaba en un envoltorio, el icono sin `[size]` no se dibujaba y `flex` no
  existía como propiedad. Los cuatro casos están arreglados; el que no avise
  sigue ahí, y es el que los hace caros de encontrar.
- **No hay *fast refresh*.** La recarga conserva el estado —la ruta, el scroll,
  lo que se declare con `hotState`— pero recrea los componentes. El de React
  Native conserva los propios componentes, y para eso hace falta cargar los
  módulos por separado y sustituir con `ɵɵreplaceMetadata` los que cambiaron.
- **En iPad se ven dos barras de pestañas.** Desde iOS 26, una `UITabBar` suelta
  —fuera de un `UITabBarController`— adopta sola la presentación flotante del
  iPad y se dibuja arriba además de en el marco que le da el layout. Fijarle una
  apariencia no la convence. En iPhone sale una sola y en su sitio.
- **Tres controles de Android no son del sistema.** La barra de pestañas, el
  control segmentado y el de pasos no están en la plataforma —viven en la
  librería de Material, que este build no usa— y se dibujan con vistas del
  sistema. Los botones de Material 3 tampoco están: la píldora se dibuja sobre
  un `Button` de verdad.
- **`VirtualList` exige altura de fila fija.** Sin ella no se puede saber qué
  hay en un desplazamiento sin haber medido todo lo anterior.
- **Faltan mapa y vídeo.** Y un selector de fecha que no sea el compacto.

## Desarrollo

`NaiveMeasurer` aproxima la medición de texto para que el núcleo sea testeable
sin plataforma. En el dispositivo mandan `UikitMeasurer` y `JniMeasurer`, que
cachean por (texto, fuente, ancho disponible): el layout mide cada nodo varias
veces por frame y sin caché eso son cientos de cruces de frontera.

Las herramientas se descubren solas: el SDK de Android por `ANDROID_HOME` o la
ruta estándar, y dentro de él la última versión de build-tools y de plataforma.
El NDK y los flags de bindgen están fijados en `.cargo/config.toml`.
