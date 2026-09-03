package dev.angularnative;

import android.app.Activity;
import android.content.Context;
import android.content.pm.PackageManager;
import android.os.Build;
import android.os.VibrationEffect;
import android.os.Vibrator;
import android.os.VibratorManager;

import org.json.JSONObject;

/**
 * The {@code haptics} module on Android and Wear OS.
 *
 * <p>It is {@link VibrationEffect}, which is the platform's own vocabulary for this: the predefined
 * effects —{@code EFFECT_TICK}, {@code EFFECT_CLICK}, {@code EFFECT_HEAVY_CLICK}, {@code
 * EFFECT_DOUBLE_CLICK}— are tuned by the manufacturer for that device's motor, and a duration in
 * milliseconds invented here would feel like a different device on every phone.
 *
 * <p>Below API 29 there are no predefined effects and below API 26 there is no {@link
 * VibrationEffect} at all, so the two older paths fall back to a one-shot of the right length. That
 * is a real difference in what is felt and it is the platform's, not a shortcut: the alternative
 * would be refusing to vibrate on a phone that can.
 *
 * <p>It needs {@code VIBRATE} in the manifest. Like {@code ACCESS_NETWORK_STATE} it is an
 * install-time permission — no dialog, nothing the person grants later — so this checks for it and
 * answers with the line to paste rather than throwing {@code SecurityException} at the first tap.
 */
public final class AnHaptics implements AnBuiltinModule {

    private Vibrator vibrator;
    private boolean permitted;

    @Override
    @SuppressWarnings("deprecation")
    public void attach(Activity host) {
        permitted =
                host.checkSelfPermission(android.Manifest.permission.VIBRATE)
                        == PackageManager.PERMISSION_GRANTED;
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            VibratorManager manager =
                    (VibratorManager) host.getSystemService(Context.VIBRATOR_MANAGER_SERVICE);
            vibrator = manager == null ? null : manager.getDefaultVibrator();
        } else {
            vibrator = (Vibrator) host.getSystemService(Context.VIBRATOR_SERVICE);
        }
    }

    @Override
    public void call(String method, JSONObject args, AnBuiltinCall respond) {
        if ("support".equals(method)) {
            support(respond);
            return;
        }
        if (!permitted) {
            respond.reject(
                    "haptics cannot vibrate: this app has no VIBRATE permission. It is an"
                            + " install-time permission, so there is no dialog to show and nothing"
                            + " the person can grant at run time — add this line to your"
                            + " AndroidManifest.xml, above <application>:\n"
                            + "  <uses-permission android:name=\"android.permission.VIBRATE\" />");
            return;
        }
        if (vibrator == null || !vibrator.hasVibrator()) {
            respond.reject(
                    "this device has no vibrator: android.os.Vibrator says so. Ask"
                            + " haptics.support() before relying on it.");
            return;
        }
        switch (method) {
            case "impact":
                impact(args.optString("style", "medium"), respond);
                return;
            case "notification":
                notification(args.optString("type", "success"), respond);
                return;
            case "selection":
                play(predefined(VibrationEffect.EFFECT_TICK, 10));
                respond.resolve();
                return;
            default:
                respond.reject("the haptics module has no method " + method);
        }
    }

    private void support(AnBuiltinCall respond) {
        JSONObject support = new JSONObject();
        try {
            support.put("available", permitted && vibrator != null && vibrator.hasVibrator());
            support.put("notification", true);
            String caveat = "";
            if (!permitted) {
                caveat = "this app has no VIBRATE permission, so nothing will be felt";
            } else if (vibrator == null || !vibrator.hasVibrator()) {
                caveat = "this device has no vibrator";
            } else if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
                caveat =
                        "below Android 10 there are no predefined effects, so the weights are"
                                + " durations chosen here rather than tuned by the manufacturer";
            }
            support.put("caveat", caveat);
        } catch (org.json.JSONException error) {
            respond.reject("haptics.support could not be built: " + error);
            return;
        }
        respond.resolve(support);
    }

    private void impact(String style, AnBuiltinCall respond) {
        switch (style) {
            case "light":
            case "soft":
                play(predefined(VibrationEffect.EFFECT_TICK, 10));
                break;
            case "medium":
            case "rigid":
                play(predefined(VibrationEffect.EFFECT_CLICK, 20));
                break;
            case "heavy":
                play(predefined(VibrationEffect.EFFECT_HEAVY_CLICK, 40));
                break;
            default:
                respond.reject(
                        "haptics.impact does not know the style "
                                + style
                                + ": it is one of light, medium, heavy, soft or rigid");
                return;
        }
        respond.resolve();
    }

    private void notification(String type, AnBuiltinCall respond) {
        // Android has no success/warning/error patterns of its own, so each is
        // built out of what there is. Unlike the Mac, here the three do come out
        // as three different things — one tick, two, and a long heavy click —
        // so the meanings survive.
        switch (type) {
            case "success":
                play(predefined(VibrationEffect.EFFECT_CLICK, 20));
                break;
            case "warning":
                play(predefined(VibrationEffect.EFFECT_DOUBLE_CLICK, 40));
                break;
            case "error":
                playPattern(new long[] {0, 40, 60, 80});
                break;
            default:
                respond.reject(
                        "haptics.notification does not know the type "
                                + type
                                + ": it is one of success, warning or error");
                return;
        }
        respond.resolve();
    }

    /**
     * The manufacturer's own effect where the platform has them, and a one-shot of {@code
     * fallbackMs} where it does not.
     */
    @SuppressWarnings("deprecation")
    private VibrationEffect predefined(int effect, long fallbackMs) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            return VibrationEffect.createPredefined(effect);
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            return VibrationEffect.createOneShot(fallbackMs, VibrationEffect.DEFAULT_AMPLITUDE);
        }
        return null;
    }

    @SuppressWarnings("deprecation")
    private void play(VibrationEffect effect) {
        if (effect == null) {
            // Below API 26 there is no VibrationEffect at all and the only way
            // in is the deprecated duration call.
            vibrator.vibrate(20);
            return;
        }
        vibrator.vibrate(effect);
    }

    @SuppressWarnings("deprecation")
    private void playPattern(long[] pattern) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            vibrator.vibrate(VibrationEffect.createWaveform(pattern, -1));
        } else {
            vibrator.vibrate(pattern, -1);
        }
    }
}
