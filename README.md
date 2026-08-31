# angular-native

React Native, pero para Angular, y con el núcleo en Rust.

Angular zoneless con signals corriendo en un motor JS embebido; el árbol de UI,
el layout y el montaje sobre **vistas nativas reales** (UIKit, Android View) los
lleva Rust. Sin DOM, sin WebView, sin zone.js.

```text
Angular (JS)  ──Renderer2──▶  cola de comandos  ──▶  ShadowTree (Rust)
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
| `an-bridge` | Motor JS y protocolo de comandos (pendiente) |
| `an-cli` | Dev server, bundling y HMR (pendiente) |

## Estado

- [x] **Fase 0** — Núcleo Rust: shadow tree, layout flexbox, diff incremental, tests
- [x] **Fase 1** — Host iOS con UIKit, medición de texto real y app en el simulador
- [ ] **Fase 2** — Motor JS embebido y puente de comandos
- [ ] **Fase 3** — Plataforma Angular: `Renderer2`, scheduler por vsync, primitivas
- [ ] **Fase 4** — CLI, dev server y HMR
- [ ] **Fase 5** — ScrollView, TextInput, listas recicladas, router, Android

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

## Desarrollo

```bash
cargo test              # núcleo completo, sin simulador ni Xcode
./scripts/run-ios.sh    # compila, enlaza y lanza la app en el simulador
```

`run-ios.sh` no usa `.xcodeproj`: compila el core con `cargo`, enlaza el shell
Swift con `swiftc` contra el `staticlib`, arma el `.app` a mano y lo instala con
`simctl`. Es lo que `an-cli` acabará haciendo. Variables: `DEVICE`, `PROFILE`.

`NaiveMeasurer` aproxima la medición de texto para que el núcleo sea testeable
sin plataforma. En el simulador manda `UikitMeasurer`, que pregunta a UIKit con
`boundingRectWithSize:` y cachea por (texto, fuente, ancho disponible): el
layout mide cada nodo tres veces por frame y sin caché eso son cientos de
cruces a Objective-C.
