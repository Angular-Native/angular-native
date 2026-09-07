---
title: "@angular-native/plugin-biometrics"
description: "Face ID, Touch ID y BiometricPrompt, como plugin de angular-native."
sidebar:
  label: biometrics
  order: 5
---

Face ID, Touch ID y `BiometricPrompt`. La comprobación es del sistema, el
diálogo es del sistema, y lo que vuelve es si la persona lo superó — nunca el
dato biométrico, que no sale del enclave.

```bash
npm install @angular-native/plugin-biometrics
```

## Dónde funciona

| iOS · iPadOS | macOS | watchOS | Android |
|:--:|:--:|:--:|:--:|
| ✓ | ✓ | — | ✓ |

Donde no funciona, el motivo es del propio plugin y lo dice la compilación, no
la primera llamada en tiempo de ejecución:

**watchOS** — un reloj no tiene sensor biométrico, y LocalAuthentication lo dice en la cabecera: LAPolicyDeviceOwnerAuthenticationWithBiometrics es API_UNAVAILABLE(watchos), y también lo son biometryNotAvailable, biometryNotEnrolled y biometryLockout. Lo que watchOS sí tiene es el código y, desde watchOS 9, LAPolicyDeviceOwnerAuthenticationWithWristDetection, que responde «este reloj está en la muñeca en la que se desbloqueó» — una comprobación real, pero no biométrica, y devolver éxito desde ella bajo un método llamado authenticate sería mentir sobre qué se verificó. Para proteger un secreto en un reloj, usa @angular-native/plugin-keychain: su mitad de watchOS guarda a través de los mismos Keychain Services y puede atar un elemento al código del dispositivo.

## La API

| Método | Devuelve | |
|---|---|---|
| `availability()` | `Promise<BiometricAvailability>` | Si se puede pedir biometría, sin pedirla: no presenta ningún diálogo.  |
| `authenticate(request: BiometricRequest)` | `Promise<BiometricResult>` |  |

Cada método es una llamada que cruza el puente, así que todos devuelven una
promesa, y todo fallo es un rechazo que nombra qué se pidió. Mira
[Módulos nativos](/es/reference/native-modules/) para saber qué cruza y qué
aspecto tiene un rechazo.
