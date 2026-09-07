---
title: El contrato
description: Seis props, un vocabulario, y tres APIs de plataforma muy distintas debajo — para qué sirve cada una, qué sale gratis, y por qué los checks leen el árbol de vuelta en vez de dar por buenas las props.
sidebar:
  order: 1
---

La mayor parte de la accesibilidad aquí no te toca a ti. `an-switch` **es** un
`UISwitch` y un `MaterialSwitch`, así que VoiceOver ya lo llama interruptor, ya
dice si está encendido, ya responde al doble toque y ya crece con el tamaño de
texto del usuario. Ese es el mayor argumento que hay para usar los controles de
la plataforma en vez de dibujar imitaciones: un interruptor dibujado le contesta
a la API de accesibilidad con silencio.

Lo que queda es la parte que ningún sistema puede deducir: cómo se *llama* un
`an-view` que hace de botón, y en qué estado está. Seis props lo cubren, son las
mismas seis en las ocho plataformas, y viven en la clase base, así que las tiene
cada primitiva.

| Prop | Tipo | A qué contesta |
|---|---|---|
| `accessibilityLabel` | `string` | ¿Cómo se llama? |
| `accessibilityRole` | `NativeRole` | ¿Qué es? |
| `accessibilityValue` | `string` | ¿Cuánto vale ahora mismo? |
| `accessibilityState` | `NativeAccessibilityState` | ¿En qué estado está? |
| `accessibilityHint` | `string` | ¿Qué pasa si lo activo? |
| `accessible` | `boolean` | ¿Esto es un elemento, o un contenedor en el que entrar? |

```html
<an-view
  [accessibilityRole]="'button'"
  [accessibilityLabel]="'Borrar esta nota'"
  [accessibilityHint]="'No se puede deshacer'"
  [accessibilityState]="{ disabled: saving() }"
  (press)="remove()">
  <an-icon [name]="'delete'" />
</an-view>
```

Sin la etiqueta, esa vista es un elemento sin nombre: puedes enfocarlo y no
puedes saber qué hace. El icono de dentro no ayuda — un icono no tiene texto.

## El vocabulario

**Roles** — doce, y ninguno más. Un nombre fuera de la lista se ignora en vez de
convertirse en un rol que sí se aplica:

`button` · `link` · `header` · `image` · `text` · `checkbox` · `radio` ·
`switch` · `slider` · `search` · `summary` · `none`

**Estado** — cinco campos opcionales, cada uno de los cuales puede sencillamente
no estar:

```ts
{ disabled?: boolean
  selected?: boolean
  checked?: boolean | 'mixed'
  expanded?: boolean
  busy?: boolean }
```

El estado se mantiene aparte del rol porque cambia con el tiempo y el rol no. Un
lector de pantalla vuelve a anunciar «seleccionado» cuando esto cambia, sin que
se reconstruya ninguna vista.

`checked` admite `'mixed'` además de un booleano, porque una casilla de tres
estados es un control real en las dos plataformas y colapsarla a `false` diría
algo que no es.

## `accessible` es la que se le escapa a la gente

Decide si un subárbol es **una parada** para un lector de pantalla o varias.

```html
<an-view [accessible]="true" [accessibilityLabel]="'Ada Lovelace, 3 sin leer'">
  <an-icon [name]="'account'" />
  <an-text>Ada Lovelace</an-text>
  <an-text>3 sin leer</an-text>
</an-view>
```

Sin ella, esa fila son tres paradas distintas, y recorrer una lista de cuarenta
son ciento veinte deslizamientos. Con ella, una parada, leída de una vez.

`false` hace lo contrario: oculta la vista **y todo lo que lleva dentro**, que es
lo que necesita la decoración pura — un separador, una imagen de fondo, un
degradado.

## Tres APIs, un contrato

Los nombres de las props son el contrato; en qué se convierten es asunto de cada
plataforma.

| | La API de debajo |
|---|---|
| **iOS · iPadOS · tvOS · visionOS** | `UIAccessibility` — rasgos, etiquetas y valores sobre `UIView` |
| **macOS** | `NSAccessibility` — un protocolo sobre `NSView`, con otro vocabulario de roles |
| **watchOS** | Los `.accessibilityLabel`, `.accessibilityValue` y `.accessibilityAddTraits` de SwiftUI |
| **Android · Wear OS** | `AccessibilityNodeInfo`, rellenado en dos mitades |

No encajan limpiamente, y donde no encajan la página de cada plataforma lo dice
en vez de disimular:

- [Accesibilidad en Apple](/es/accessibility/apple/) — las tres formas de Apple,
  rol por rol y estado por estado, y qué no tiene equivalente en cada una.
- [Accesibilidad en Android](/es/accessibility/android/) — las dos mitades de
  `AccessibilityNodeInfo`, el mapeo de roles, y el bug de orden que resultó tener
  `accessible`.

## `testID` no es una de las seis

```html
<an-view [testID]="'save-button'" />
```

Acaba en `accessibilityIdentifier`, que es lo que buscan los frameworks de tests
de interfaz. No lo lee nada en voz alta y no es una etiqueta: una vista con
`testID` y sin `accessibilityLabel` sigue sin tener nombre para un lector de
pantalla.

## Los checks leen el árbol de vuelta

`scripts/check-accessibility.sh` y `scripts/check-a11y.sh` **no** comprueban que
las props se hayan puesto. Eso solo demostraría que la plantilla dice lo que dice
la plantilla.

Instalan la app y le piden al sistema su árbol de accesibilidad —por la misma API
que usan VoiceOver y TalkBack— y comparan lo que de verdad se le contaría a un
lector de pantalla. Después lo comparan con lo que pedía la plantilla.

Eso es otra afirmación, y la única que merece la pena hacer. Una prop que viaja
hasta un host que no la lee se ve idéntica a una que funciona, hasta que alguien
con un lector de pantalla prueba la app.

La mitad que no cuesta nada corre en `check-all.sh`; `check-a11y-device.sh` es la
mitad que instala un APK en un móvil de verdad. Ver
[comprobarlo sin dispositivo](/es/guide/testing/).

## Lo que el volcado no puede enseñar

Leer el árbol demuestra que la etiqueta está y que dice las palabras correctas.
No demuestra que el orden tenga sentido, que el foco vaya a algún sitio útil
después de cerrar un modal, ni que la pista merezca la pena leerla. Eso necesita
una persona y un lector de pantalla, y las dos páginas de plataforma dicen cuáles
de sus afirmaciones son de ese tipo.
