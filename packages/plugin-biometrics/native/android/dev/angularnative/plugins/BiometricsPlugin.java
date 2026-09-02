package dev.angularnative.plugins;

import android.app.Activity;
import android.hardware.biometrics.BiometricManager;
import android.hardware.biometrics.BiometricPrompt;
import android.os.Build;
import android.os.CancellationSignal;

import dev.angularnative.AnPlugin;
import dev.angularnative.AnPluginCall;

import org.json.JSONException;
import org.json.JSONObject;

/**
 * Biometría de Android: {@code BiometricPrompt}.
 *
 * <p>El de la plataforma, {@code android.hardware.biometrics.BiometricPrompt}, no el de
 * {@code androidx.biometric}. Aquí no hay Gradle: las dependencias de Android están vendidas una a
 * una en {@code vendor/android}, y meter una librería entera para envolver una API que ya trae el
 * sistema sería pagar el APK dos veces. El precio de la decisión está dicho abajo: por debajo de
 * Android 10 este plugin contesta que no puede, en vez de callarse.
 *
 * <p>El diálogo lo dibuja el sistema por encima de la app. Este plugin no pinta nada y no ve nunca
 * la huella ni la cara: solo recibe cómo acabó.
 */
public final class BiometricsPlugin implements AnPlugin {

    /**
     * {@code BiometricPrompt} llegó en Android 9 (API 28), pero
     * {@code BiometricManager.canAuthenticate} —lo único que sabe decir si hay sensor y si hay algo
     * registrado sin enseñar un diálogo— llegó en Android 10 (API 29).
     *
     * <p>Se pide 29 y no 28 a propósito. Con 28 habría que averiguar la disponibilidad enseñando el
     * diálogo y mirando qué error sale, y `availability()` dejaría de ser lo que dice ser: una
     * pregunta que no interrumpe a nadie.
     */
    private static final int MINIMO = Build.VERSION_CODES.Q;

    private Activity host;

    /**
     * La cancelación del diálogo que esté en pantalla, si hay alguno.
     *
     * <p>Sin esto, una segunda llamada mientras la primera sigue abierta deja dos diálogos del
     * sistema encima del otro y una promesa que ya no le va a llegar nada. Con esto, la primera se
     * cierra y contesta {@code systemCancel} antes de que la segunda empiece.
     */
    private CancellationSignal pending;

    @Override
    public void attach(Activity host) {
        this.host = host;
    }

    @Override
    public void call(String method, JSONObject args, AnPluginCall respond) {
        switch (method) {
            case "availability":
                respond.resolve(availability());
                return;

            case "authenticate":
                String reason = args.optString("reason", "");
                if (reason.isEmpty()) {
                    respond.reject(
                            "biometrics.authenticate necesita un 'reason' no vacío: es el título"
                                    + " del diálogo del sistema");
                    return;
                }
                authenticate(
                        reason,
                        args.optString("subtitle", null),
                        args.optString("cancelTitle", null),
                        args.optBoolean("allowDeviceCredential", false),
                        respond);
                return;

            default:
                respond.reject("el plugin biometrics no tiene ningún método " + method);
        }
    }

    // ── Disponibilidad ──────────────────────────────────────────────────────

    /**
     * Qué hay y si se puede usar, sin enseñar nada.
     *
     * <p>El {@code kind} es siempre {@code unknown} o {@code none}: {@code BiometricManager} dice si
     * se puede autenticar, no con qué. iOS sí lo dice, y por eso el contrato lo lleva; rellenarlo
     * aquí con {@code fingerprint} porque la mayoría de los Android lo son sería mentir justo en los
     * que llevan cámara.
     */
    private JSONObject availability() {
        if (Build.VERSION.SDK_INT < MINIMO) {
            return estado(
                    "unavailable",
                    "none",
                    "este plugin necesita Android 10 (API " + MINIMO + ") y el aparato tiene API "
                            + Build.VERSION.SDK_INT);
        }
        BiometricManager manager =
                host == null ? null : host.getSystemService(BiometricManager.class);
        if (manager == null) {
            return estado("unavailable", "none", "el sistema no da BiometricManager");
        }
        int codigo = manager.canAuthenticate();
        switch (codigo) {
            case BiometricManager.BIOMETRIC_SUCCESS:
                return estado("available", "unknown", "BIOMETRIC_SUCCESS");
            case BiometricManager.BIOMETRIC_ERROR_NO_HARDWARE:
                return estado("noHardware", "none", "BIOMETRIC_ERROR_NO_HARDWARE");
            case BiometricManager.BIOMETRIC_ERROR_NONE_ENROLLED:
                return estado("notEnrolled", "unknown", "BIOMETRIC_ERROR_NONE_ENROLLED");
            case BiometricManager.BIOMETRIC_ERROR_HW_UNAVAILABLE:
                return estado("unavailable", "unknown", "BIOMETRIC_ERROR_HW_UNAVAILABLE");
            default:
                // API 30 añadió BIOMETRIC_ERROR_SECURITY_UPDATE_REQUIRED, y las
                // que vengan detrás llegarán aquí. El número dentro es cierto;
                // dar `available` sería enseñar un botón que no funciona.
                return estado("unavailable", "unknown", "canAuthenticate devolvió " + codigo);
        }
    }

    // ── Autenticación ───────────────────────────────────────────────────────

    private void authenticate(
            String reason,
            String subtitle,
            String cancelTitle,
            boolean allowDeviceCredential,
            AnPluginCall respond) {
        if (Build.VERSION.SDK_INT < MINIMO) {
            respond.resolve(
                    resultado(
                            "unavailable",
                            "none",
                            "este plugin necesita Android 10 (API " + MINIMO + ") y el aparato"
                                    + " tiene API " + Build.VERSION.SDK_INT));
            return;
        }
        if (host == null) {
            respond.reject("el plugin biometrics no tiene contexto de Android");
            return;
        }

        BiometricPrompt.Builder builder = new BiometricPrompt.Builder(host).setTitle(reason);
        if (subtitle != null && !subtitle.isEmpty()) {
            builder.setSubtitle(subtitle);
        }
        if (allowDeviceCredential && Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            builder.setAllowedAuthenticators(
                    BiometricManager.Authenticators.BIOMETRIC_STRONG
                            | BiometricManager.Authenticators.DEVICE_CREDENTIAL);
        } else {
            // Sin botón negativo, `BiometricPrompt` lanza al construirse. Y con
            // `DEVICE_CREDENTIAL` puesto, ponerlo lanza también: el sistema pone
            // el suyo. De ahí que sea un `else` y no una línea suelta.
            builder.setNegativeButton(
                    cancelTitle == null || cancelTitle.isEmpty() ? "Cancelar" : cancelTitle,
                    host.getMainExecutor(),
                    (dialog, which) -> {
                        // No se contesta aquí: el sistema manda además
                        // ERROR_NEGATIVE_BUTTON por el callback de error, y
                        // contestar dos veces dejaría la segunda en el aire.
                    });
        }

        if (pending != null && !pending.isCanceled()) {
            // Cierra el diálogo anterior. Su callback recibirá ERROR_CANCELED y
            // contestará `systemCancel`, que es exactamente lo que pasó.
            pending.cancel();
        }
        CancellationSignal signal = new CancellationSignal();
        pending = signal;
        builder.build()
                .authenticate(
                        signal,
                        host.getMainExecutor(),
                        new BiometricPrompt.AuthenticationCallback() {
                            @Override
                            public void onAuthenticationSucceeded(
                                    BiometricPrompt.AuthenticationResult authResult) {
                                pending = null;
                                respond.resolve(
                                        resultado("success", "unknown", "onAuthenticationSucceeded"));
                            }

                            @Override
                            public void onAuthenticationError(int code, CharSequence message) {
                                pending = null;
                                respond.resolve(
                                        resultado(
                                                translate(code),
                                                "unknown",
                                                "BiometricPrompt error " + code + ": " + message));
                            }

                            @Override
                            public void onAuthenticationFailed() {
                                // Un intento que no reconoce a nadie. **No se
                                // contesta**: el diálogo sigue en pantalla y el
                                // usuario puede volver a probar. Contestar aquí
                                // cerraría la promesa con el diálogo todavía
                                // abierto, y la siguiente respuesta —la buena o
                                // el bloqueo— no tendría a quién llegar.
                            }
                        });
    }

    /**
     * De los códigos de {@code BiometricPrompt} a los nombres del contrato.
     *
     * <p>Android sí separa el bloqueo temporal del permanente, y iOS no. Es la diferencia que hace
     * que {@code permanentlyLockedOut} solo salga de aquí.
     */
    private static String translate(int code) {
        switch (code) {
            case BiometricPrompt.BIOMETRIC_ERROR_HW_UNAVAILABLE:
            case BiometricPrompt.BIOMETRIC_ERROR_NO_SPACE:
            case BiometricPrompt.BIOMETRIC_ERROR_VENDOR:
                return "unavailable";
            case BiometricPrompt.BIOMETRIC_ERROR_UNABLE_TO_PROCESS:
                return "failed";
            case BiometricPrompt.BIOMETRIC_ERROR_TIMEOUT:
                return "timeout";
            case BiometricPrompt.BIOMETRIC_ERROR_CANCELED:
                return "systemCancel";
            case BiometricPrompt.BIOMETRIC_ERROR_LOCKOUT:
                return "lockedOut";
            case BiometricPrompt.BIOMETRIC_ERROR_LOCKOUT_PERMANENT:
                return "permanentlyLockedOut";
            case BiometricPrompt.BIOMETRIC_ERROR_USER_CANCELED:
            case BiometricPrompt.BIOMETRIC_ERROR_NEGATIVE_BUTTON:
                return "userCancel";
            case BiometricPrompt.BIOMETRIC_ERROR_NO_BIOMETRICS:
                return "notEnrolled";
            case BiometricPrompt.BIOMETRIC_ERROR_HW_NOT_PRESENT:
                return "noHardware";
            case BiometricPrompt.BIOMETRIC_ERROR_NO_DEVICE_CREDENTIAL:
                return "passcodeNotSet";
            default:
                // API 30 añadió BIOMETRIC_ERROR_SECURITY_UPDATE_REQUIRED, y las
                // que vengan detrás llegarán aquí con su número en `detail`.
                return "unavailable";
        }
    }

    /** Lo que devuelve {@code availability()}: su clave es {@code status}. */
    private static JSONObject estado(String status, String kind, String detail) {
        return json("status", status, kind, detail);
    }

    /** Lo que devuelve {@code authenticate()}: su clave es {@code outcome}. */
    private static JSONObject resultado(String outcome, String kind, String detail) {
        return json("outcome", outcome, kind, detail);
    }

    /**
     * El objeto de vuelta. {@code kind} y {@code detail} van siempre: una respuesta a la que le
     * falta {@code detail} obliga a comprobarlo en cada uso del otro lado.
     */
    private static JSONObject json(String clave, String valor, String kind, String detail) {
        JSONObject json = new JSONObject();
        try {
            json.put(clave, valor);
            json.put("kind", kind);
            json.put("detail", detail);
        } catch (JSONException error) {
            // `put` de una cadena no nula no lanza nunca; si lo hiciera, un
            // objeto a medias sería peor que decirlo.
            throw new IllegalStateException("no se pudo armar la respuesta de biometrics", error);
        }
        return json;
    }
}
