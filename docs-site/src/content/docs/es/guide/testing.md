---
title: Comprobarlo sin dispositivo
description: Un comando lo corre todo, `headless` renderiza un bundle sin plataforma debajo, y un puñado de scripts vigilan las listas que están duplicadas en tres lenguajes.
sidebar:
  order: 9
---

Casi todo lo que puede salir mal aquí sale mal **en silencio**. Una prop que
ningún host reconoce no levanta nada: viaja, nadie la lee, y el control se queda
como estaba. Una primitiva cuyo nombre se ha separado entre el renderer y el
protocolo se monta como una vista envoltorio de más. Ninguna de las dos sale como
error; las dos salen como «esto no hace nada», buscado en el sitio equivocado, a
razón de una tarde cada una.

Así que la suite no es sobre todo tests unitarios. Es sobre todo *scripts que
leen dos ficheros y exigen que sigan estando de acuerdo*, más un renderer que
mueve la cadena entera sin ninguna plataforma debajo.

```bash
./scripts/check-all.sh
```

Eso es todo lo que se puede verificar sin dispositivo: los tests de Rust, las
listas duplicadas, las apps de ejemplo, los árboles de accesibilidad, los
plugins, un proyecto de Angular de fuera del repositorio, un `.app` de macOS de
verdad que se lanza y se captura, y las dos compilaciones cruzadas. Corren unos
treinta scripts y cada uno imprime sus propias líneas `ok`.

Un script que no puede hacer su trabajo imprime **`skipped`**, no un aprobado.
Esa distinción es el objetivo: una suite que se pone verde porque el emulador no
estaba arrancado es peor que una que se pone roja.

## `headless`

```bash
cargo an build examples/hello-angular
cargo run -p an-bridge --example headless -- build/bundle/hello-angular/main.js 6
```

Monta la cadena entera menos la plataforma: QuickJS evalúa el bundle, el árbol en
la sombra recibe las mutaciones, taffy resuelve el layout, el diff produce las
operaciones de montaje — y el host es una grabadora que apunta lo que le dan en
vez de crear vistas.

El reloj es falso. `6` es un número de frames, y un tercer argumento opcional son
los milisegundos que avanza cada uno. Como aquí JS no tiene reloj propio —los
timers avanzan con el vsync— un test puede simular diez segundos sin esperar diez
segundos, y obtiene la misma respuesta siempre.

Simula además una pulsación, un arrastre, un scroll y un volver, y después
imprime el árbol resuelto:

```text
View#1 [0,0 393x852]
  Text#2 [16,115 361x33] "angular-native"
  View#3 [16,164 361x88]
    View#4 [0,0 116x88]
    View#5 [128,0 233x88]
-- frame 4 (t=4000ms): 1 operation
```

Cada posición de ese volcado es el número que calculó el núcleo, y por eso los
scripts comprueban frames exactos y no «se ha renderizado». Es la forma más
rápida de depurar una plantilla: sin simulador, sin construir un `.app`, en
aproximadamente un segundo.

La grabadora apunta deliberadamente **todas las props que le dan**, no solo las
que cambian la geometría. Sin eso, una prop que llega bien y una que no llega
nunca se ven idénticas en un volcado —ninguna mueve nada— y comprobar que
`[variant]` o `[ios]` han viajado sería imposible sin dispositivo.

## Las listas que no pueden separarse

Tres listas existen en más de un lenguaje porque no queda otra, y tres scripts
existen por eso.

| Script | Qué compara |
|---|---|
| `check-kinds.sh` | El selector de la directiva → `NATIVE_KINDS` en el renderer → `KIND` en el preludio → `kind_from_byte` en Rust. Cuatro eslabones, un nombre. |
| `check-styles.sh` | La lista de nombres de estilo de JS contra la del núcleo. |
| `check-wrapper.sh` | Cada prop que declara una directiva contra los hosts que se supone que la leen — incluido que una clave `[ios]` aparezca en el host de iOS y *no* en el de Android. |

Son scripts que comparan cadenas, no tienen ningún glamour, y son los tests de
más valor del repositorio. `check-kinds.sh` en particular es lo que permite que
la regla etiqueta→nombre siga siendo una regla en vez de convertirse en una tabla
de traducción que alguien tiene que acordarse de actualizar.

`check-signals.sh` es de la misma familia, y prohíbe en vez de comparar: nada de
`@Input`, ni `@Output`, ni `EventEmitter`, ni `@ViewChild`, ni `@HostBinding`. No
por gusto: un `@Input() set` corre en el momento exacto en que Angular escribe el
input, así que el orden de las escrituras sigue al orden de los bindings de la
plantilla, mientras que una señal se lee cuando alguien la lee. Mezclar las dos
en un mismo árbol es que una prop llegue en distinto orden según quién escribiera
la plantilla.

## Correr una sola cosa

Cada script se sostiene solo:

```bash
./scripts/check-kinds.sh          # solo la cadena de nombres
./scripts/check-angular.sh        # la cadena entera, sobre hello-angular
./scripts/check-list.sh           # 5.000 filas, y que el scroll no crea vistas
./scripts/check-clip.sh           # que el cuerpo recorta y nada hace scroll de lado
./scripts/check-macos.sh          # compila un .app, lo lanza y lo captura
```

Algunos admiten una app:

```bash
./scripts/check-angular.sh examples/controls
```

Y el lado de Rust es cargo normal:

```bash
cargo test                  # todo el workspace
cargo test -p an-core       # un crate
cargo test -p an-watch snapshot
```

Los crates de host compilan vacíos fuera de su plataforma, que es lo que mantiene
`cargo test` funcionando en un Mac — y también lo que significa que un
`cargo test` verde no demuestra que el host de iOS compile. De eso se encarga la
compilación cruzada del final de `check-all.sh`.

## La mitad que necesita hardware

Cinco scripts no están en `check-all.sh` porque no pueden estarlo:

| Script | Por qué necesita un dispositivo |
|---|---|
| `check-a11y-device.sh` | Instala el APK y le pide al sistema su árbol de accesibilidad real. |
| `check-builtins-device.sh` | Háptico, compartir y demás, donde el hardware es la respuesta. |
| `check-keyboard-device.sh` | La altura del teclado es la de la plataforma, no la del simulador. |
| `check-measure-device.sh` | Medición de texto con las fuentes reales del dispositivo y su ajuste de tamaño. |
| `check-rotation-device.sh` | La rotación y los márgenes que vienen con ella. |

Imprimen `skipped` con su motivo cuando no hay nada enchufado, y dicen *cuál* es
el motivo: un móvil bloqueado dice que está bloqueado en vez de dejar que la
culpa se la lleve el código.

## Los checks de accesibilidad no son afirmaciones de intención

`check-accessibility.sh` y `check-a11y.sh` no comprueban que las props se hayan
puesto. Eso solo demostraría que la plantilla dice lo que dice la plantilla.

Instalan la app y le piden al sistema su árbol de accesibilidad —por la misma API
que usan VoiceOver y TalkBack— y comparan lo que de verdad se le contaría a un
lector de pantalla. Eso es otra afirmación, y la única que merece la pena hacer.
Ver [el contrato](/es/accessibility/overview/) para en qué se convierten las seis
props en cada plataforma.

## Añadir un check

La convención es corta y merece seguirse: un asunto por script, líneas `ok` y
`FAIL` con una descripción en castellano llano, `skipped` cuando faltan los
requisitos, y salida distinta de cero solo con `FAIL`. Casi todos son veinte
líneas de bash alrededor de un `grep` sobre la salida de `headless`.

```bash
OUTPUT="$(cargo run -q -p an-bridge --example headless -- build/bundle/mia/main.js 6 2>&1)"
check 'Switch#[0-9]+ .*on=true' 'el interruptor llegó encendido'
```

Y después se añade a `scripts/check-all.sh`, en el grupo que le corresponda.
