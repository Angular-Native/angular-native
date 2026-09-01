package dev.angularnative;

import android.util.Log;

import org.json.JSONArray;
import org.json.JSONObject;

/**
 * La respuesta de una llamada a un plugin.
 *
 * <p>Se puede contestar desde cualquier hilo: al otro lado hay un buzón con cerrojo, no una
 * variable del hilo de UI. Lo que no se puede es no contestar — la promesa del lado JS se queda
 * esperando para siempre—, así que todo camino de error tiene que acabar en {@link #reject}.
 */
public final class AnPluginCall {

    private static final String TAG = "angular-native";

    private final long id;
    private boolean answered;

    AnPluginCall(long id) {
        this.id = id;
    }

    /** Para un método que no devuelve nada. */
    public void resolve() {
        send("null");
    }

    public void resolve(String text) {
        // `JSONObject.quote` escapa y pone las comillas: es el escape de JSON
        // que trae la plataforma, sin envolver la cadena en un objeto.
        send(text == null ? "null" : JSONObject.quote(text));
    }

    public void resolve(boolean value) {
        send(value ? "true" : "false");
    }

    public void resolve(double value) {
        send(Double.toString(value));
    }

    public void resolve(JSONObject object) {
        send(object == null ? "null" : object.toString());
    }

    public void resolve(JSONArray array) {
        send(array == null ? "null" : array.toString());
    }

    public void reject(String message) {
        synchronized (this) {
            if (answered) {
                return;
            }
            answered = true;
        }
        AnRuntime.pluginReject(id, message == null ? "el plugin falló" : message);
    }

    private void send(String json) {
        synchronized (this) {
            if (answered) {
                Log.w(TAG, "un plugin contestó dos veces a la misma llamada");
                return;
            }
            answered = true;
        }
        AnRuntime.pluginResolve(id, json);
    }
}
