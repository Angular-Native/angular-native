---
title: "@angular-native/plugin-camera"
description: "La cámara y la fototeca, presentadas por el sistema, como plugin de angular-native."
sidebar:
  label: camera
  order: 7
---

La cámara y la fototeca, **presentadas** en lugar de incrustadas:
`UIImagePickerController` y `PHPickerViewController` en iOS, el intent de cámara
y el selector de fotos en Android.

Nada cruza el puente como bytes. Una fotografía de doce megapíxeles en base64
son dieciséis megabytes de cadena atravesando una frontera JSON, codificados una
vez y parseados otra; se escribe el fichero y vuelve su ruta.

La fototeca no necesita permiso en iOS 14 ni en Android 13 y posteriores: esos
selectores se ejecutan fuera de la app y devuelven solo lo que se eligió.

```bash
npm install @angular-native/plugin-camera
```

## Dónde funciona

| iOS · iPadOS | macOS | watchOS | Android |
|:--:|:--:|:--:|:--:|
| ✓ | ✓ | — | ✓ |

Donde no funciona, el motivo es del propio plugin y lo dice la compilación, no
la primera llamada en tiempo de ejecución:

**watchOS** — un reloj no tiene cámara, y watchOS no trae ni UIImagePickerController ni PHPickerViewController. Lo que sí puede hacer es pedirle al teléfono emparejado que tome una, que es otra funcionalidad con otra API.

## La API

| Método | Devuelve | |
|---|---|---|
| `permission()` | `Promise<CameraPermission>` |  |
| `request()` | `Promise<CameraPermission>` |  |
| `takePhoto(options: PhotoOptions = {})` | `Promise<Photo>` | Abre la cámara y resuelve con lo que se tomó. |
| `pickPhoto(options: PhotoOptions = {})` | `Promise<Photo>` | Abre la fototeca. |

Cada método es una llamada que cruza el puente, así que todos devuelven una
promesa, y todo fallo es un rechazo que nombra qué se pidió. Mira
[Módulos nativos](/es/reference/native-modules/) para saber qué cruza y qué
aspecto tiene un rechazo.
