---
title: Llevar un proyecto Angular al móvil
description: Ya tienes una app de `ng new` y quieres que corra sobre vistas nativas. No hace falta mover nada a este repositorio.
---

Este documento es para quien ya tiene una app de Angular —una de `ng new`— y
quiere que corra sobre vistas nativas. No hace falta clonar nada dentro de este
repositorio ni mover el proyecto a `examples/`: `an` sirve igual desde fuera.

## De principio a fin

```bash
# 1. El binario, una vez por máquina. Deja `an` en el PATH y se acuerda de dónde
#    está el SDK, así que desde aquí ya no hay que volver a este directorio.
cd angular-native
cargo install --path crates/an-cli

# 2. Un proyecto Angular cualquiera.
npx @angular/cli@latest new mi-app
cd mi-app

# 3. Prepararlo.
an init

# 4. La plataforma.
an add ios

# 5. Al simulador.
an ios
```

`an dev` en vez de `an ios` levanta además el servidor de desarrollo y recarga la
app al guardar, con el estado de la pantalla intacto.

## Qué deja `an init` en el proyecto

```text
mi-app/
  angular-native.json           el manifiesto: nombre, identificador y entrada
  .angular-native/
    tsconfig.json               el tsconfig del build nativo
    vendor/*.tgz                los dos paquetes del framework, empaquetados
    build/                      el .app, el APK y el JS compilado   (gitignored)
  ios/Info.plist                lo escribe `an add ios`; a partir de ahí es tuyo
  android/AndroidManifest.xml   lo escribe `an add android`; ídem
  src/main.native.ts            el arranque de la app nativa
  src/app/app-native.ts         su componente raíz
```

Y en el `package.json`, dos dependencias nuevas:

```json
"@angular-native/platform": "file:.angular-native/vendor/angular-native-platform-0.0.1.tgz",
"@angular-native/primitives": "file:.angular-native/vendor/angular-native-primitives-0.0.1.tgz"
```

Nada más. La app web sigue exactamente como estaba: `ng build` y `ng serve`
funcionan igual, porque `an` no toca ni `src/main.ts`, ni `angular.json`, ni el
`tsconfig.json` del proyecto.

## Dos raíces, dos árboles de componentes

La app nativa arranca por `src/main.native.ts` y la web por `src/main.ts`. Los
servicios, los modelos, las señales y el estado se comparten sin más —son
TypeScript normal—; **las plantillas no**. Un `<div>` no tiene equivalente
nativo, y `<an-view>` no se puede pintar en un navegador. Compartir plantillas
exigiría un lenguaje intermedio que se tradujera a los dos, que es exactamente lo
que este proyecto decidió no hacer: aquí un `<an-switch>` es un `UISwitch`, con
el aspecto y el comportamiento que tenga en esa versión de iOS, y eso no se puede
prometer y a la vez pintar en HTML.

Así que el reparto queda:

| Se comparte | No se comparte |
|---|---|
| servicios, modelos, señales, validaciones, cliente HTTP | plantillas, estilos CSS, todo lo que dependa del DOM |

## Las tres decisiones

### De dónde salen `@angular-native/platform` y `@angular-native/primitives`

Los dos paquetes son de este repositorio y todavía no están publicados en npm.
Había tres formas de meterlos en un proyecto de fuera:

- **Una ruta local**, `file:../angular-native/packages/primitives`. Es la más
  cómoda de escribir y la peor de todas: npm la instala como enlace simbólico, la
  ruta es la del disco de quien ejecutó `an init`, y el `package-lock.json` que se
  commitea no le sirve a nadie más del equipo.
- **Un tarball vendorizado**: `npm pack` dentro del proyecto, con su versión, y
  la dependencia apuntando a él. Es una copia exacta de lo que el SDK tenía ese
  día. Se commitea con el proyecto, `npm ci` la reinstala sin red y sin el SDK
  delante, y el build no se puede desincronizar del framework sin que alguien lo
  vea en un diff.
- **Publicarlos en npm**, que es lo que hará falta el día que esto salga del
  cajón.

Elegido el segundo. Y el día que se publiquen solo cambia el especificador: en
`node_modules` queda exactamente lo mismo, así que nada de lo que hay por encima
—ni el tsconfig, ni esbuild, ni el bundle— se entera del cambio.

Lo que se empaqueta **no son las fuentes**: son los dos paquetes compilados, con
sus `.d.ts` y en modo parcial, que es como se publica cualquier librería de
Angular. Aquí hay una trampa que costó encontrar: TypeScript **no emite
JavaScript para las fuentes que están bajo `node_modules`**, las da por librería
externa ya compilada. Instalar los `.ts` y apuntar el tsconfig a ellos con
`paths` compila sin una sola queja y produce un bundle al que le falta medio
framework, que esbuild descubre tres pasos más tarde. Compilarlos antes de
empaquetarlos convierte ese silencio en el caso aburrido: se resuelven como
cualquier otra dependencia y el Angular Linker de `scripts/bundle.mjs` hace el
resto.

Se compilan con el `ngc` **del proyecto**, no con el del SDK, para que los
`.d.ts` y las declaraciones parciales salgan de la misma versión de Angular
contra la que se compila la app.

### Qué hace `an init` si el proyecto no es Angular, o si ya está inicializado

Ninguna de las dos puede acabar en un estropicio a medias, y cada una se resuelve
de una forma distinta.

**Si no es Angular**, no empieza. `an init` mira `angular.json` y `@angular/core`
en el `package.json` antes de tocar nada, y si falta alguno lo dice por su nombre
y explica que esto se ejecuta dentro de un proyecto ya creado. No hay estado
intermedio porque no llegó a escribir nada.

**Si ya está inicializado**, sigue adelante y solo añade lo que falte. Un fichero
que ya existe se deja y se dice que se ha dejado; `--force` reescribe lo que
generamos nosotros —el tsconfig, y reinstala los paquetes— pero nunca el código
de la app. Correr `an init` dos veces es seguro por definición, y correrlo
después de actualizar el SDK es la forma de traerse los paquetes nuevos.

Lo que hace que las dos cosas se sostengan es el orden: **`angular-native.json`
se escribe el último**. Todo lo que puede fallar —npm, `ngc`, el SDK a medias—
pasa antes. Si algo se tuerce, el manifiesto no llega a existir, `an` sigue
diciendo que el proyecto no está inicializado, y volver a intentarlo es seguro.
No hay un estado «medio inicializado» que alguien tenga que limpiar a mano.

### Dónde viven los proyectos nativos, y qué pasa si los editas

Capacitor crea `ios/` y `android/` como proyectos completos de Xcode y de Gradle,
los declara propiedad del usuario, y `npx cap sync` solo vuelve a copiar los
assets web y a registrar los plugins. Es la respuesta correcta *para Capacitor*:
sin el `.xcodeproj` no hay build, así que el proyecto tiene que existir y alguien
tiene que ser su dueño. El precio es un fichero de miles de líneas que nadie
revisa en un diff y que se desincroniza en silencio.

Aquí no hay `.xcodeproj` ni Gradle —`an` invoca `swiftc`, `aapt2`, `d8` y
`apksigner` directamente— así que no hace falta que exista ningún proyecto
nativo. La respuesta es partir en dos lo que Capacitor deja junto:

- **Configuración: tuya, y se commitea.** `ios/Info.plist` y
  `android/AndroidManifest.xml`. Los crea `an add`, y a partir de ese momento
  `an` los copia al build y **no los reescribe nunca**. Ahí van los permisos, las
  orientaciones, las claves de privacidad y lo que la app declare. Son ficheros
  de cincuenta líneas que se leen en un diff.
- **El `.app` y el APK: producto del build.** Viven en
  `.angular-native/build/`, están en el `.gitignore` y se rehacen enteros en cada
  compilación. Editarlos ahí no tiene sentido y no hay que decidir qué pasa si
  alguien lo hace: se borran solos en el siguiente `an ios`.

Queda una manera de que las dos mitades se separen, y está tapada: si cambias
`app.name` o `app.bundleId` en `angular-native.json` y no tocas el plist, iOS
instala una app que busca un ejecutable que no está y desaparece al abrirla, sin
un solo error. `an ios` compara las dos cosas **antes de compilar nada** y se
para diciendo cuál es cuál. Lo mismo con el `package` del `AndroidManifest.xml`,
que tiene que seguir siendo el de las clases del shell: el identificador de la
aplicación lo pone `aapt2` con `--rename-manifest-package`.

## Dónde busca `an` el SDK

Fuera del monorepo, `an` necesita saber dónde están los crates y los shells. Dos
sitios, en este orden:

1. `AN_HOME`, si está definida.
2. La ruta desde la que se compiló el binario. `cargo install --path
   crates/an-cli` la deja grabada dentro, así que un `an` en el PATH sabe volver
   a su repositorio.

Si el sitio existe pero le falta algo, se dice qué falta. Por eso
`angular-native.json` **no** guarda la ruta del SDK: sería la del disco de quien
ejecutó `an init`, y el fichero se commitea.

## Comprobarlo sin simulador

```bash
./scripts/check-external.sh
```

Monta un proyecto Angular de verdad en `build/check-external/`, lo pasa por
`an init`, `an add ios`, `an add android` y `an build`, y corre el bundle
resultante en el `headless`. Comprueba también las tres formas que esto tiene de
estropear el proyecto de otro: `an init` sobre algo que no es Angular, un
`an init` repetido que pisara código escrito a mano, y un `Info.plist`
desincronizado del manifiesto. Va dentro de `./scripts/check-all.sh`.
