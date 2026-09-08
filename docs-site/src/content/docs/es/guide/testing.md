---
title: Comprobarlo sin dispositivo
description: Un comando lo corre todo, `headless` renderiza un bundle sin plataforma debajo, y un puñado de scripts vigilan las listas que están duplicadas en tres lenguajes.
sidebar:
  order: 10
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

## Lo que no llegó a correr

Una línea `--` en medio de dos mil se pasa por alto, así que la ejecución
termina con una sección `== skipped` que las cuenta y las repite:

```
== skipped
       cross-compilation skipped: the nightly toolchain is missing
       the signed macOS build is skipped: this keychain has no codesigning identity
  --   2 steps did not run; the reason is printed where each one happened.
```

La otra salida es `ok   nothing was skipped: every step ran`. Es un informe y no
una puerta: `--no-android`, una máquina sin runtime de simulador y un llavero sin
identidad de firma se saltan por buenos motivos, y una suite que fallara con
cualquiera de ellos dejaría de poder correrse en un portátil.

El salto que no tiene buen motivo es el de las compilaciones cruzadas de nivel 3
de Apple. `aarch64-apple-tvos-sim`, `aarch64-apple-visionos-sim` y
`aarch64-apple-watchos-sim` no traen `std` precompilada, así que `-Z build-std`
la construye a partir de `rust-src`, que solo existe en nightly;
`rust-toolchain.toml` fija stable, porque un fichero de toolchain fija un canal y
los otros siete crates no pintan nada en nightly. Un solo comando cubre los tres,
y es el que ejecuta CI:

```bash
rustup toolchain install nightly --component rust-src
```

Por eso el job de CI busca `cross-compilation skipped` en su propia salida y
falla si lo encuentra. El runner instala ese toolchain; un salto ahí no es una
máquina a la que le falta algo, son tres targets que no compila nadie.

## Las dos mitades

```bash
./scripts/check-all.sh               # todo
./scripts/check-all.sh --no-android  # todo menos la mitad de abajo
./scripts/check-android.sh           # el shell Java, Wear OS, las dos compilaciones cruzadas de Android
```

La mitad de Android es un script aparte porque quiere una máquina distinta del
resto: las comprobaciones de Apple necesitan swiftc, los SDK de los simuladores y
un `.app` de verdad, y las de Android necesitan el NDK y nada de Apple.
`check-all.sh` llama a `check-android.sh` en vez de repetir lo que hay dentro, así
que hay una lista de cada mitad y ninguna puede separarse de la que corre CI.

## En CI

`.github/workflows/ci.yml` corre esos dos comandos en dos runners: la mitad de
Apple en `macos-15`, la de Android en `ubuntu-24.04`. Las dos imágenes llevan el
SDK de Android y el NDK, así que el Mac podría hacerlo todo — no lo hace porque
un minuto de macOS se factura a diez veces uno de Linux, y nada de la mitad de
Android necesita un Mac.

El trabajo del Mac instala una cosa antes que nada: `rustup toolchain install
nightly --component rust-src`. Los targets de iOS y de Android salen de
`rust-toolchain.toml`; nightly no puede, y ese paso es la única razón por la que
las tres compilaciones cruzadas de nivel 3 de Apple corren en algún sitio.
Después el trabajo busca `cross-compilation skipped` en la salida de la propia
suite y falla si lo encuentra, para que el día que ese paso deje de funcionar la
ejecución se ponga roja en vez de dejar caer tres targets sin decirlo.

Tres cosas que el trabajo de Linux tiene que acertar, y ninguna es una suposición
sobre el runner:

- **El NDK** ya está en la imagen, así que no hay paso de `sdkmanager`. La imagen
  pone `ANDROID_NDK_HOME` en su NDK por defecto, que es lo que mantiene a `an`
  lejos de los más nuevos que también están instalados. Antes de compilar nada, el
  trabajo comprueba que el compilador que nombra `an env android` existe de
  verdad: un NDK que hubiera dejado caer el nivel de API contra el que se compila
  el core aparecería si no como un linker que falta dentro de un crate que no
  tiene nada que ver.
- **El nombre del host.** `find_ndk_toolchain` lee el único directorio que hay
  bajo `toolchains/llvm/prebuilt/` en vez de dar por hecho `darwin-x86_64`, así
  que encuentra `linux-x86_64` sin cambiar nada.
- **Material 3**, que `fetch-android-deps.py` resuelve a mano: un centenar de
  viajes a dos repositorios Maven para traer 43 MB, en cada ejecución, incluso
  cuando todos los jars ya están en disco. `vendor/android/` se cachea, con la
  clave puesta en los dos scripts que deciden qué acaba dentro y en la versión de
  build-tools cuyo `aapt2` compiló los recursos — las tres cosas que cambian su
  contenido.

Un cuarto trabajo arranca un emulador y corre `check-a11y-device.sh` encima, con
`AN_ABI=x86_64` para que el APK lleve la arquitectura que el emulador es. Es
`continue-on-error` y no bloquea `main`: es la única comprobación que lee el árbol
de accesibilidad desde fuera, de un Android de verdad, y a la vez la señal menos
fiable del fichero — un emulador alojado que no arranca pondría la ejecución en
rojo por algo que no está en el código, y una ejecución roja que nadie se cree es
peor que ninguna.

:::caution[El workflow no se ha ejecutado nunca]
Cada paso suyo se ha corrido a mano en un Mac — `an env android` con una
toolchain `linux-x86_64` puesta en disco, una descarga de dependencias en frío, el
shell Java, Wear OS, las dos compilaciones cruzadas, un APK `x86_64` — pero el
fichero entero no ha corrido nunca en un runner de GitHub. Lo que nadie ha visto:
el primer fallo de caché, un Linux de verdad, y el emulador arrancando.
:::

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
