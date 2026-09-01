package dev.angularnative.plugins;

import android.app.Activity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;

import dev.angularnative.AnPlugin;
import dev.angularnative.AnPluginCall;

import org.json.JSONObject;

/**
 * El portapapeles de Android.
 *
 * <p>Se compila dentro del APK en la misma invocación de {@code javac} que el shell, así que ve
 * {@link AnPlugin} y {@link AnPluginCall} sin classpath adicional. Quien lo registra es el fichero
 * que genera {@code an} a partir del {@code package.json}.
 *
 * <p>{@code ClipboardManager} quiere el hilo de UI, y aquí ya estamos en él: el core entrega las
 * llamadas dentro del frame. Por eso se puede contestar en el acto en vez de guardarse el
 * {@link AnPluginCall} para más tarde.
 */
public final class ClipboardPlugin implements AnPlugin {

    /** La pantalla de la app. Un plugin de Android casi siempre necesita un contexto. */
    private Activity host;

    @Override
    public void attach(Activity host) {
        this.host = host;
    }

    @Override
    public void call(String method, JSONObject args, AnPluginCall respond) {
        ClipboardManager clipboard =
                host == null
                        ? null
                        : (ClipboardManager) host.getSystemService(Context.CLIPBOARD_SERVICE);
        if (clipboard == null) {
            respond.reject("el plugin clipboard no tiene contexto de Android");
            return;
        }

        switch (method) {
            case "write":
                String text = args.optString("text", null);
                if (text == null) {
                    respond.reject("clipboard.write necesita un texto en 'text'");
                    return;
                }
                clipboard.setPrimaryClip(ClipData.newPlainText("angular-native", text));
                respond.resolve();
                return;

            case "read":
                // Cadena vacía y no nulo: quien pide el portapapeles quiere
                // pintar algo, y `undefined` obligaría a comprobarlo en cada uso.
                respond.resolve(readText(clipboard));
                return;

            case "hasText":
                respond.resolve(clipboard.hasPrimaryClip() && !readText(clipboard).isEmpty());
                return;

            default:
                // Nunca en silencio: un método que no existe rechaza la promesa
                // diciendo cuál se pidió.
                respond.reject("el plugin clipboard no tiene ningún método " + method);
        }
    }

    private static String readText(ClipboardManager clipboard) {
        ClipData clip = clipboard.getPrimaryClip();
        if (clip == null || clip.getItemCount() == 0) {
            return "";
        }
        CharSequence text = clip.getItemAt(0).getText();
        return text == null ? "" : text.toString();
    }
}
