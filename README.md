# angular-native

React Native, pero para Angular, y con el núcleo en Rust.

Angular zoneless con signals corriendo en un motor JS embebido; el árbol de UI,
el layout y el montaje sobre **vistas nativas reales** los lleva Rust. Sin DOM,
sin WebView, sin zone.js.

```text
Angular (JS)  ──Renderer2──▶  __an_dom  ──▶  búfer binario  ──▶  ShadowTree (Rust)
                                                                      │ commit
                                                                taffy (layout)
                                                                      │ diff
                                                                Frame { MountOp[] }
                                                                      ▼
                                                    HostRenderer (UIKit / android.view)
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
      <View [backgroundColor]="'#1e2a4a'" (press)="onPress($event)"></View>
      @if (seconds() >= 3) {
        <Text [fontSize]="14">han pasado {{ seconds() }} segundos</Text>
      }
    </View>
  `
})
export class AppComponent {
  readonly seconds = signal(0)
  onPress(event: NativePressEvent) { /* ... */ }
}
```

```ts
bootstrapNativeApplication(AppComponent)
```

`<View>` acaba siendo una `UIView` en iOS y una `AnViewGroup` en Android.

## Empezar

```bash
cargo an dev                  # compila, lanza en el simulador y recarga al guardar
cargo an ios                  # una sola vez, sin vigilar
cargo an android              # APK, emulador y lanzamiento
cargo an build --release      # solo el bundle: 276 KB frente a 933 KB en debug
```

Todo va por el mismo binario, `an`. No hay `.xcodeproj` ni Gradle: las
herramientas de cada SDK ya hacen el trabajo y el proceso cabe en un fichero
que se puede leer entero.

## Verificación sin dispositivo

```bash
cargo test                    # núcleo Rust
./scripts/check-angular.sh    # la cadena entera: ngc, esbuild, QuickJS, taffy
./scripts/check-list.sh       # ScrollView, TextInput, lista con ventana, módulo nativo
./scripts/check-router.sh     # navegación y parámetros de ruta
cargo run -p an-bridge --example headless -- build/bundle/hello-angular/main.js 6
```

`headless` monta el pipeline entero salvo la plataforma: evalúa un bundle,
avanza frames con un reloj falso, simula un toque e imprime el árbol resuelto.
Es la forma rápida de depurar sin simulador, y es lo que usan los tres scripts.

## Crates

| Crate | Qué hace |
|---|---|
| `an-layout` | Props de estilo a `taffy::Style`, árbol de layout, medición de hojas |
| `an-core` | Shadow tree, mutaciones, commit, diff hacia `MountOp` |
| `an-host` | Traits `HostRenderer` y `TextMeasurer`, y el `Renderer` que los une |
| `an-bridge` | Motor JS (QuickJS), protocolo binario y módulos nativos |
| `an-ios` | Host UIKit, medidor con CoreText y superficie C |
| `an-android` | Host JNI, medidor con `StaticLayout` y puntos de entrada JNI |
| `an-cli` | La herramienta `an`: build, ios, android y servidor de desarrollo |

| Paquete npm | Qué hace |
|---|---|
| `packages/runtime` | Prelude JS: consola, temporizadores, `AbortController`, escritor de comandos |
| `packages/platform-native` | `Renderer2`, plataforma, `PlatformLocation` y módulos nativos |
| `packages/primitives` | `View`, `Text`, `Image`, `ScrollView`, `TextInput` y `VirtualList` |

## Estado

- [x] **Núcleo Rust** — shadow tree, layout flexbox, diff incremental
- [x] **iOS** — UIKit, medición de texto real, gestos, app en el simulador
- [x] **Motor JS** — QuickJS, protocolo binario, `main.js` en el bundle
- [x] **Angular** — `Renderer2`, primitivas tipadas, AOT, zoneless, eventos
- [x] **CLI** — build, ios, android y recarga en caliente
- [x] **ScrollView, TextInput y listas con ventana**
- [x] **Router** sobre una pila de navegación en memoria
- [x] **Módulos nativos** con macro y servicio tipado
- [x] **Android** — host JNI, APK sin Gradle, emulador

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
- **Búfer binario, no llamadas sueltas.** Un `*ngFor` de 200 filas son ~1.200
  mutaciones. Con llamadas por mutación son 1.200 cruces de frontera; así es uno.
- **JS no tiene hilo, tiene un turno por frame.** El `CADisplayLink` —o el
  `Choreographer`— llama a `tick()`, y ahí dentro corren temporizadores y
  microtareas hasta agotarlas. El reloj de `setTimeout` es el del vsync, así que
  el tiempo de la app es determinista y un test puede simular diez segundos sin
  esperarlos.
- **AOT siempre, nunca JIT.** `ngc` compila las plantillas en el build y el
  Angular Linker resuelve los paquetes publicados en modo parcial; el
  dispositivo no lleva `@angular/compiler`.
- **Primitivas como directivas tipadas, no `CUSTOM_ELEMENTS_SCHEMA`.** El
  esquema laxo exige un guion en el nombre y, peor, apaga la comprobación de
  propiedades: `[bakcgroundColor]` con errata pasaría el compilador y fallaría
  en silencio en el dispositivo.
- **Los eventos se registran solo si la plantilla los pide.** `(press)` es una
  salida sobre un observable frío: el reconocedor de gestos se engancha al
  suscribirse. Una vista que nadie escucha no paga nada.
- **`onLayout` lo emite el core**, no la plataforma: es él quien calcula el
  marco, así que funciona igual en iOS y en Android sin implementarlo dos veces.
- **Un ScrollView no se dimensiona por su contenido.** Sin ese default, una
  lista de cinco mil filas produce un ScrollView de 280.000 puntos de alto y se
  lleva por delante el layout del padre.
- **Nada de `@angular/platform-browser`.** Arrastra `DomAdapter`,
  `DomRendererFactory2` y el sanitizador de HTML, todo asumiendo que existe un
  DOM. La plataforma propia son ~70 líneas.

## Lo que no está hecho

Cosas que se descubrieron construyendo esto y que hay que resolver antes de
llamarlo listo para producción:

- **El motor JS necesita ~4 MB de pila.** El router de Angular encadena
  diecisiete operadores de RxJS y la recursión de subscripción es profunda. El
  hilo principal de iOS tiene 1 MB y no se puede cambiar: la solución real es
  mover el motor y el árbol a un hilo propio —lo que hace React Native— y dejar
  en el de UI solo el montaje. La costura ya existe: `Frame { MountOp[] }` es
  serializable. Lo que hay que resolver con ella es la medición de texto, que
  hoy pregunta a UIKit y tendría que pasar a CoreText, que sí es thread-safe.
- **Ventana, no reciclado.** `VirtualList` monta las filas visibles y destruye
  las que salen; no reutiliza vistas como un `UITableView`. Reciclar exige
  reasignar el contexto de una vista de Angular ya creada.
- **Una vista nativa por componente.** El host de un componente Angular se
  monta como `View`. Fabric aplana esas vistas en una pasada posterior
  (*view flattening*); aquí todavía no.
- **El router no tiene navegación nativa.** La ruta cambia y la vista se
  sustituye, sin `UINavigationController`, sin animación y sin gesto de volver
  atrás. `NativePlatformLocation.back()` es el punto por donde entrarían.
- **La recarga en caliente pierde el estado.** El *fast refresh* de React
  Native exige saber qué componentes cambiaron y reconciliar el árbol.
- **Android va por detrás de iOS.** El host monta vistas, mide texto y entrega
  toques, pero le faltan gestos más allá del `press`, eventos de scroll y de
  campo de texto, y el cliente del servidor de desarrollo.

## Desarrollo

`NaiveMeasurer` aproxima la medición de texto para que el núcleo sea testeable
sin plataforma. En el dispositivo mandan `UikitMeasurer` y `JniMeasurer`, que
cachean por (texto, fuente, ancho disponible): el layout mide cada nodo varias
veces por frame y sin caché eso son cientos de cruces de frontera.

Las herramientas se descubren solas: el SDK de Android por `ANDROID_HOME` o la
ruta estándar, y dentro de él la última versión de build-tools y de plataforma.
El NDK y los flags de bindgen están fijados en `.cargo/config.toml`.
