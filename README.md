# angular-native

React Native, pero para Angular, y con el núcleo en Rust.

Angular zoneless con signals corriendo en un motor JS embebido; el árbol de UI,
el layout y el montaje sobre **vistas nativas reales** (UIKit, Android View) los
lleva Rust. Sin DOM, sin WebView, sin zone.js.

```text
Angular (JS)  ──Renderer2──▶  __an_dom  ──▶  búfer binario  ──▶  ShadowTree (Rust)
                                                                      │ commit
                                                                taffy (layout)
                                                                      │ diff
                                                                Frame { MountOp[] }
                                                                      ▼
                                                         HostRenderer (UIKit / Android)
```

Tres hilos: JS (lógica), core Rust (árbol, layout, diff), UI (vistas nativas).

## Crates

| Crate | Qué hace |
|---|---|
| `an-layout` | Props de estilo a `taffy::Style`, árbol de layout, medición de hojas |
| `an-core` | Shadow tree, mutaciones, commit, diff hacia `MountOp` |
| `an-host` | Traits `HostRenderer` y `TextMeasurer`, y el `Renderer` que los une |
| `an-ios` | Host UIKit, medidor de texto y superficie C para el shell |
| `an-bridge` | Motor JS (QuickJS) y protocolo binario de comandos |
| `an-cli` | La herramienta `an`: build, ios y servidor de desarrollo |

| Paquete npm | Qué hace |
|---|---|
| `packages/runtime` | Prelude JS: consola, temporizadores, `requestAnimationFrame`, escritor de comandos |
| `packages/platform-native` | `Renderer2`, `RendererFactory2` y `bootstrapNativeApplication` |
| `packages/primitives` | `View`, `Text`, `Image`, `ScrollView`, `TextInput` como directivas tipadas |

## Estado

- [x] **Fase 0** — Núcleo Rust: shadow tree, layout flexbox, diff incremental, tests
- [x] **Fase 1** — Host iOS con UIKit, medición de texto real y app en el simulador
- [x] **Fase 2** — Motor JS embebido, protocolo binario y `main.js` en el bundle
- [x] **Fase 3** — Plataforma Angular: `Renderer2`, primitivas tipadas, AOT, zoneless y eventos
- [x] **Fase 4** — CLI `an`: build, ios y recarga en caliente
- [ ] **Fase 5** — ScrollView, TextInput, listas recicladas, router, Android

## Una app

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
}
```

```ts
bootstrapNativeApplication(AppComponent)
```

Señales, `@if`, `@for` y bindings son los de Angular, sin cambios. Lo único
distinto es que `<View>` y `<Text>` acaban siendo `UIView` y `UILabel`.

## Decisiones ya tomadas

- **Vistas nativas, no pintado propio.** Accesibilidad, IME, scroll y look del
  sistema salen gratis; el coste es una capa por plataforma para cada primitiva.
- **Los ids de nodo los asigna JS.** Crear un nodo no necesita viaje de vuelta
  al core, igual que los tags de Fabric.
- **Layout en `taffy`**, no Yoga: es Rust, y trae flexbox, grid y block.
- **Zoneless obligatorio.** Sin zone.js no hay que parchear timers ni XHR
  dentro del motor JS, que es el 80% del dolor de NativeScript.
- **Un commit por frame.** El layout solo corre en `commit()`, y el `Frame`
  resultante lleva únicamente lo que cambió.
- **Búfer binario, no llamadas sueltas.** Un `*ngFor` de 200 filas son ~1.200
  mutaciones. Con llamadas por mutación son 1.200 cruces de frontera; con búfer
  es uno.
- **JS no tiene hilo, tiene un turno por frame.** El `CADisplayLink` llama a
  `tick()`, y ahí dentro corren temporizadores y microtareas hasta agotarlas.
  El reloj de `setTimeout` es el del vsync, así que el tiempo de la app es
  determinista y un test puede simular diez segundos sin esperarlos.
- **El script viaja en el bundle**, no compilado dentro de Rust, igual que el
  `main.jsbundle` de React Native.
- **AOT siempre, nunca JIT.** `ngc` compila las plantillas en el build; el
  dispositivo no lleva `@angular/compiler`.
- **Primitivas como directivas tipadas, no `CUSTOM_ELEMENTS_SCHEMA`.** El
  esquema laxo exige un guion en el nombre y, peor, apaga la comprobación de
  propiedades: `[bakcgroundColor]` con errata pasaría el compilador y fallaría
  en silencio en el dispositivo. Con directivas cada prop es un `@Input`
  declarado, comprobado y autocompletado.
- **Los eventos se registran solo si la plantilla los pide.** `(press)` es una
  salida de la directiva sobre un observable frío: el `UIGestureRecognizer` se
  engancha al suscribirse. Una vista que nadie escucha no paga nada.
- **Nada de `@angular/platform-browser`.** Arrastra `DomAdapter`,
  `DomRendererFactory2` y el sanitizador de HTML: todo asume que existe un DOM.
  La plataforma propia son ~60 líneas.

## Desarrollo

```bash
cargo an dev                  # compila, lanza en el simulador y recarga al guardar
cargo an ios                  # una sola vez, sin vigilar
cargo an build --release      # solo el bundle: 276 KB frente a 933 KB en debug

cargo test                    # núcleo Rust, sin simulador ni Xcode
./scripts/check-angular.sh    # la cadena entera: ngc, esbuild, QuickJS, taffy
cargo run -p an-bridge --example headless -- build/bundle/hello-angular/main.js 6
```

`an dev` vigila las fuentes, recompila el bundle y avisa a la app por
WebSocket; la app se lo descarga y se reinicia sola, sin volver a pasar por
Xcode. La recarga es completa: el estado se pierde. Preservarlo —el *fast
refresh* de React Native— exige saber qué componentes cambiaron y reconciliar
el árbol, y es un proyecto en sí mismo.

El `headless` evalúa un bundle, avanza frames con un reloj falso, simula un
toque e imprime el árbol resuelto. Es la forma rápida de depurar el lado JS
sin simulador.

`run-ios.sh` no usa `.xcodeproj`: compila el core con `cargo`, enlaza el shell
Swift con `swiftc` contra el `staticlib`, arma el `.app` a mano y lo instala con
`simctl`. Es lo que `an-cli` acabará haciendo. Variables: `DEVICE`, `PROFILE`.

`NaiveMeasurer` aproxima la medición de texto para que el núcleo sea testeable
sin plataforma. En el simulador manda `UikitMeasurer`, que pregunta a UIKit con
`boundingRectWithSize:` y cachea por (texto, fuente, ancho disponible): el
layout mide cada nodo tres veces por frame y sin caché eso son cientos de
cruces a Objective-C.
