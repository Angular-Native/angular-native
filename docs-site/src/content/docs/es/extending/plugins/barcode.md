---
title: "@angular-native/plugin-barcode"
description: "Leer un código de barras con la cámara, a pantalla completa, a través de AVFoundation — como plugin de angular-native."
sidebar:
  label: barcode
  order: 10
---

Leer un código de barras con la cámara. `AVCaptureMetadataOutput` descodifica en
el dispositivo, sin modelo que descargar y sin servicio detrás.

Viene de dos formas. `scan()` abre a pantalla completa y se cierra cuando lee
algo. Y hay una **previsualización en vivo que puedes colocar en tu propio
layout**, que es la única vista que aporta un plugin en este proyecto — mira
[una vista que trae un plugin](/es/extending/plugins/#una-vista-que-trae-un-plugin):

```html
<an-custom [view]="'barcode-preview'" [style.height]="'260'" [borderRadius]="16" />
```

La previsualización emite lo que lee por el canal de eventos del módulo en lugar
de resolver una promesa, porque una previsualización produce ninguno o cien, y
una promesa lleva uno.

```bash
npm install @angular-native/plugin-barcode
```

## Dónde funciona

| iOS · iPadOS | macOS | watchOS | Android |
|:--:|:--:|:--:|:--:|
| ✓ | ✓ | — | — |

Donde no funciona, el motivo es del propio plugin y lo dice la compilación, no
la primera llamada en tiempo de ejecución:

**watchOS** — un reloj no tiene cámara. Las clases de captura de AVFoundation ni siquiera están en el SDK de watchOS.

**Android** — Android no trae ninguna API de códigos de barras. La respuesta habitual es ML Kit, que vive en los Google Play Services y es una dependencia de Gradle; esta cadena de compilación no tiene Gradle, y empaquetar la variante que depende de Play produciría una app que no se puede instalar en un dispositivo sin Play Services. La alternativa es incrustar un descodificador como ZXing en el shell, que es una decisión sobre lo que carga *toda* app de angular-native, y no una que un plugin pueda tomar por su cuenta.

## La API

| Método | Devuelve | |
|---|---|---|
| `available()` | `Promise<boolean>` |  |
| `scan(options: ScanOptions = {})` | `Promise<Barcode>` | Abre el escáner y resuelve con lo primero que lee. |

Cada método es una llamada que cruza el puente, así que todos devuelven una
promesa, y todo fallo es un rechazo que nombra qué se pidió. Mira
[Módulos nativos](/es/reference/native-modules/) para saber qué cruza y qué
aspecto tiene un rechazo.
