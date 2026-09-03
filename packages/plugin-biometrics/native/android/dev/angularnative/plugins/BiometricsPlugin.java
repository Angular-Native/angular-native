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
 * The Android biometrics: {@code BiometricPrompt}.
 *
 * <p>The platform's one, {@code android.hardware.biometrics.BiometricPrompt}, not the one from
 * {@code androidx.biometric}. There is no Gradle here: the Android dependencies are vendored one by
 * one in {@code vendor/android}, and pulling in a whole library to wrap an API the system already
 * ships would be paying for the APK twice. The price of that decision is stated below: under
 * Android 10 this plugin answers that it cannot, instead of keeping quiet.
 *
 * <p>The dialog is drawn by the system on top of the app. This plugin paints nothing and never sees
 * the fingerprint or the face: all it gets is how it ended.
 */
public final class BiometricsPlugin implements AnPlugin {

    /**
     * {@code BiometricPrompt} arrived in Android 9 (API 28), but
     * {@code BiometricManager.canAuthenticate} —the only thing that can say whether there is a
     * sensor and whether anything is enrolled without showing a dialog— arrived in Android 10
     * (API 29).
     *
     * <p>29 is asked for rather than 28 on purpose. With 28 availability would have to be worked out
     * by showing the dialog and looking at which error comes back, and `availability()` would stop
     * being what it says it is: a question that interrupts nobody.
     */
    private static final int MINIMUM_API = Build.VERSION_CODES.Q;

    /**
     * The code the cancel button arrives with.
     *
     * <p>The system sends it, but the constant is not public in the platform API: it is in
     * {@code androidx.biometric} as {@code ERROR_NEGATIVE_BUTTON}, and androidx is not used here. It
     * is written out with its name and its reason instead of leaving a bare 13 inside a
     * {@code case}.
     */
    private static final int ERROR_NEGATIVE_BUTTON = 13;

    private Activity host;

    /**
     * The cancellation for whichever dialog is on screen, if there is one.
     *
     * <p>Without this, a second call while the first one is still open leaves two system dialogs on
     * top of each other and a promise nothing is ever going to reach again. With it, the first one
     * closes and answers {@code systemCancel} before the second one starts.
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
                            "biometrics.authenticate needs a non-empty 'reason': it is the title"
                                    + " of the system dialog");
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
                respond.reject("the biometrics plugin has no method " + method);
        }
    }

    // ── Availability ────────────────────────────────────────────────────────

    /**
     * What there is and whether it can be used, without showing anything.
     *
     * <p>The {@code kind} is always {@code unknown} or {@code none}: {@code BiometricManager} says
     * whether it can authenticate, not with what. iOS does say, which is why the contract carries
     * it; filling it in here with {@code fingerprint} because most Androids are would be lying about
     * exactly the ones with a camera.
     */
    private JSONObject availability() {
        if (Build.VERSION.SDK_INT < MINIMUM_API) {
            return state(
                    "unavailable",
                    "none",
                    "this plugin needs Android 10 (API " + MINIMUM_API + ") and the device is on API "
                            + Build.VERSION.SDK_INT);
        }
        BiometricManager manager =
                host == null ? null : host.getSystemService(BiometricManager.class);
        if (manager == null) {
            return state("unavailable", "none", "the system does not hand out a BiometricManager");
        }
        int code = manager.canAuthenticate();
        switch (code) {
            case BiometricManager.BIOMETRIC_SUCCESS:
                return state("available", "unknown", "BIOMETRIC_SUCCESS");
            case BiometricManager.BIOMETRIC_ERROR_NO_HARDWARE:
                return state("noHardware", "none", "BIOMETRIC_ERROR_NO_HARDWARE");
            case BiometricManager.BIOMETRIC_ERROR_NONE_ENROLLED:
                return state("notEnrolled", "unknown", "BIOMETRIC_ERROR_NONE_ENROLLED");
            case BiometricManager.BIOMETRIC_ERROR_HW_UNAVAILABLE:
                return state("unavailable", "unknown", "BIOMETRIC_ERROR_HW_UNAVAILABLE");
            default:
                // API 30 added BIOMETRIC_ERROR_SECURITY_UPDATE_REQUIRED, and
                // whichever come after it will land here. The number inside is
                // true; answering `available` would mean showing a button that
                // does not work.
                return state("unavailable", "unknown", "canAuthenticate returned " + code);
        }
    }

    // ── Authentication ──────────────────────────────────────────────────────

    private void authenticate(
            String reason,
            String subtitle,
            String cancelTitle,
            boolean allowDeviceCredential,
            AnPluginCall respond) {
        if (Build.VERSION.SDK_INT < MINIMUM_API) {
            respond.resolve(
                    result(
                            "unavailable",
                            "none",
                            "this plugin needs Android 10 (API " + MINIMUM_API + ") and the device"
                                    + " is on API " + Build.VERSION.SDK_INT));
            return;
        }
        if (host == null) {
            respond.reject("the biometrics plugin has no Android context");
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
            // With no negative button, `BiometricPrompt` throws when it is built.
            // And with `DEVICE_CREDENTIAL` set, setting one throws as well: the
            // system puts its own there. Hence an `else` rather than a line on
            // its own.
            builder.setNegativeButton(
                    cancelTitle == null || cancelTitle.isEmpty() ? "Cancel" : cancelTitle,
                    host.getMainExecutor(),
                    (dialog, which) -> {
                        // No answer is given here: the system also sends
                        // ERROR_NEGATIVE_BUTTON through the error callback, and
                        // answering twice would leave the second one hanging.
                    });
        }

        if (pending != null && !pending.isCanceled()) {
            // Closes the previous dialog. Its callback will get ERROR_CANCELED
            // and answer `systemCancel`, which is exactly what happened.
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
                                        result("success", "unknown", "onAuthenticationSucceeded"));
                            }

                            @Override
                            public void onAuthenticationError(int code, CharSequence message) {
                                pending = null;
                                respond.resolve(
                                        result(
                                                translate(code),
                                                "unknown",
                                                "BiometricPrompt error " + code + ": " + message));
                            }

                            @Override
                            public void onAuthenticationFailed() {
                                // An attempt that recognises nobody. **No answer
                                // is given**: the dialog stays on screen and the
                                // user can try again. Answering here would close
                                // the promise with the dialog still open, and the
                                // next answer —the good one, or the lockout—
                                // would have nobody left to reach.
                            }
                        });
    }

    /**
     * From the {@code BiometricPrompt} codes to the names in the contract.
     *
     * <p>Android does separate the temporary lockout from the permanent one, and iOS does not. That
     * is the difference that makes {@code permanentlyLockedOut} come from here and nowhere else.
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
            case ERROR_NEGATIVE_BUTTON:
                return "userCancel";
            case BiometricPrompt.BIOMETRIC_ERROR_NO_BIOMETRICS:
                return "notEnrolled";
            case BiometricPrompt.BIOMETRIC_ERROR_HW_NOT_PRESENT:
                return "noHardware";
            case BiometricPrompt.BIOMETRIC_ERROR_NO_DEVICE_CREDENTIAL:
                return "passcodeNotSet";
            default:
                // API 30 added BIOMETRIC_ERROR_SECURITY_UPDATE_REQUIRED, and
                // whichever come after it will land here with their number in
                // `detail`.
                return "unavailable";
        }
    }

    /** What {@code availability()} returns: its key is {@code status}. */
    private static JSONObject state(String status, String kind, String detail) {
        return json("status", status, kind, detail);
    }

    /** What {@code authenticate()} returns: its key is {@code outcome}. */
    private static JSONObject result(String outcome, String kind, String detail) {
        return json("outcome", outcome, kind, detail);
    }

    /**
     * The object that goes back. {@code kind} and {@code detail} are always there: an answer missing
     * its {@code detail} forces a check at every single use on the other side.
     */
    private static JSONObject json(String key, String value, String kind, String detail) {
        JSONObject json = new JSONObject();
        try {
            json.put(key, value);
            json.put("kind", kind);
            json.put("detail", detail);
        } catch (JSONException error) {
            // `put` of a non-null string never throws; if it ever did, a
            // half-built object would be worse than saying so.
            throw new IllegalStateException("the biometrics answer could not be built", error);
        }
        return json;
    }
}
