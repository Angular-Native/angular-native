---
title: Instalar el CLI
description: "`npm install -g @angular-native/cli` deja `an` en el PATH con el SDK que necesita — qué lleva el paquete, qué tiene que tener aún tu máquina y por qué el core de Rust no se distribuye compilado."
sidebar:
  order: 2
---

```bash
npm install -g @angular-native/cli
an --version
```

Esa es toda la instalación. No hay nada que clonar, ningún `AN_HOME` que definir
y ninguna ruta que apuntar a un checkout: el paquete lleva tanto el ejecutable
`an` como el SDK del que lee.

## Qué acaba en el disco

Se publican seis paquetes y npm se descarga dos.

**`@angular-native/cli`** es la carga. Son los mismos bytes en cualquier
máquina, porque lo que lleva son *fuentes*: el Swift de las cinco shells de
Apple, el Java de las dos de Android, el Rust del core, `scripts/bundle.mjs` y
el TypeScript de `@angular-native/primitives` y `@angular-native/platform`. Unos
7 MB comprimidos, 17 MB desempaquetados, la mayor parte la fuente Material
Symbols que necesita la shell de Android.

**`@angular-native/cli-<host>`** es un ejecutable nativo y nada más. Hay cinco
—`darwin-arm64`, `darwin-x64`, `linux-arm64`, `linux-x64`, `win32-x64`— listados
como `optionalDependencies` con `os` y `cpu`, así que npm instala el que
corresponde a esta máquina y se salta los otros cuatro. Es el mismo montaje que
usan esbuild y swc, y es la razón de que la instalación sea un ejecutable y no
cinco.

El `an` que queda en el PATH es un pequeño shim de Node dentro del primer
paquete. Le pregunta al resolutor de Node dónde ha ido a parar el segundo, le
dice al ejecutable dónde está la carga y se aparta. Esa indirección no es
adorno: con pnpm los dos paquetes son directorios sin relación dentro de
`node_modules/.pnpm`, sin camino de uno a otro, y el resolutor de Node es lo
único que sabe la respuesta.

## Qué le hace falta aún a tu máquina

| Para compilar | Necesitas |
|---|---|
| el bundle — `an init`, `an build`, `an plugins` | Node 20.19 o más nuevo, y nada más |
| iOS, tvOS, visionOS, watchOS, macOS | Xcode, y [Rust](https://rustup.rs) |
| Android, Wear OS | el SDK de Android con un NDK, un JDK, y Rust |
| tvOS, visionOS, watchOS | además `rustup toolchain install nightly --component rust-src` |

En Linux y en Windows el CLI es real, pero la mitad alcanzable es Android: las
plataformas de Apple necesitan `xcrun`, `swiftc` y los simuladores, y eso solo
existe en macOS.

Android quiere además Material y las bibliotecas que arrastra, resueltas una
vez. Los dos scripts que lo hacen viajan dentro del paquete, y dejan su
resultado al lado:

```bash
cd "$(dirname "$(dirname "$(readlink -f "$(which an)")")")"
python3 scripts/fetch-android-deps.py
python3 scripts/prepare-android-deps.py
```

Eso pide que el paquete instalado se pueda escribir, cosa que un prefijo global
bajo `/usr/local` no permite. Instala el CLI en un prefijo propio —`npm config
set prefix ~/.npm-global`— o mantén un checkout y apunta `AN_HOME` a él.

El primer comando de plataforma compila el core de Rust, lo que lleva unos
minutos una vez y después queda en caché. `rust-toolchain.toml` viaja dentro del
paquete, así que rustup instala el toolchain y los cuatro targets sin que nadie
se lo pida. `cargo` deja lo que compila en el `.angular-native/build/target` de
tu proyecto, nunca dentro de `node_modules` — una instalación global está muy a
menudo donde no puedes escribir, y una local la borra `npm ci`.

## Por qué el core no se distribuye compilado

El paso siguiente evidente a «npm instala el ejecutable» es «npm instala también
el core compilado, y así nadie necesita Rust». Se sopesó y se descartó, y la
razón conviene escribirla porque parece un descuido.

El ejecutable es **por host**: cinco combinaciones, y cada máquina se descarga
una. Las bibliotecas estáticas son **por target**, y una sola máquina las
necesita todas — un Mac compila para `aarch64-apple-ios`,
`aarch64-apple-ios-sim`, dos triples de tvOS, dos de visionOS, dos de watchOS,
dos de macOS y cuatro ABIs de Android. Son catorce archivos, no cinco, y ninguno
es opcional como sí lo es un ejecutable para otro sistema operativo. Uno de
ellos, con los símbolos quitados, ronda los 40 MB, 9 MB comprimido; el conjunto
son cientos de megabytes por instalación, y eso antes de la segunda copia que
pedirían los perfiles de debug y de release.

Enfrente, lo que ahorraría: `rustup`, un comando y unos 300 MB. Y una máquina
que va a ejecutar `an ios` ya tiene Xcode encima, y una que va a ejecutar
`an android` ya tiene el NDK — los dos de varios gigabytes. Rust no es el
elemento fuera de lugar que su tamaño hace parecer; es el más pequeño de los
tres toolchains que el comando ya exige.

Hay una segunda razón, y es la que lo cierra. Un archivo precompilado tiene que
concordar con las cabeceras de `crates/an-ios/include` y con los bytes del
protocolo que intercambian las shells y el bundle. Cuando se separan no salta
nada: la app monta un árbol con un agujero. Cada lista de este proyecto que está
duplicada entre lenguajes tiene un script que la vigila, y un binario compilado
en otro sitio es una duplicación que nada puede vigilar.

Los tres targets de nivel 3 de Apple —tvOS, visionOS y watchOS— son el único
sitio donde unas bibliotecas precompiladas ayudarían de verdad, porque son la
razón de que esas plataformas pidan nightly. Si eso cambia, cambiará solo para
esos tres, y quien lo prefiera los seguirá compilando desde las fuentes.

## Los tres sitios donde `an` busca el SDK

En este orden:

1. **`AN_HOME`**, si está definido. El shim de npm lo pone apuntando a su propio
   paquete antes de lanzar el ejecutable, y lo que definas tú gana sobre eso —
   que es como se trabaja en el propio framework con un `an` que vino de npm:

   ```bash
   AN_HOME=~/src/angular-native an build
   ```

2. **La ruta desde la que se compiló el binario**, que es lo que deja grabado
   `cargo install --path crates/an-cli`. Un `an` compilado desde un checkout
   sabe volver a él.

3. **Al lado del ejecutable.** Una subida desde el binario buscando
   `@angular-native/cli`, que es lo que hace que funcione cuando se ejecuta
   directamente en vez de a través del shim.

Si el sitio existe pero le falta `packages/runtime/runtime.js`,
`scripts/bundle.mjs`, `shells` o `crates`, `an` dice cuál en lugar de fallar más
tarde dentro de `swiftc`.

## Sin npm

El CLI se puede compilar desde las fuentes en cualquier host que soporte Rust,
incluidos aquellos para los que no hay ejecutable publicado:

```bash
git clone https://github.com/Angular-Native/angular-native
cd angular-native
cargo install --path crates/an-cli
```

Ese `an` encuentra el SDK por la regla 2 de arriba, así que el checkout tiene
que quedarse donde está. Si lo mueves, define `AN_HOME`.

## Actualizar y desinstalar

```bash
npm install -g @angular-native/cli@latest
npm uninstall -g @angular-native/cli
```

Eso no actualiza los paquetes del framework dentro de un proyecto. Están
empaquetados como tarballs en `.angular-native/vendor` y se comprometen con el
proyecto a propósito — ver
[Llevar un proyecto Angular al móvil](/es/guide/existing-angular-project/). Para
mover un proyecto a una versión nueva, ejecuta `an init --force`, que los vuelve
a compilar y a empaquetar desde el SDK que ahora está en el PATH.
