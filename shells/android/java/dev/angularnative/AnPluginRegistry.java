package dev.angularnative;

import android.app.Activity;
import android.util.Log;

import org.json.JSONObject;

import java.util.HashMap;
import java.util.Map;

/**
 * El registro de plugins del APK.
 *
 * <p>Quién está dentro lo decide {@code an} al armar el APK, leyendo las dependencias del
 * {@code package.json} de la app: lo escribe en {@code AnGeneratedPlugins.java}, que es lo que
 * llama {@link #install(Activity)}.
 */
public final class AnPluginRegistry {

    private static final String TAG = "angular-native";

    private static final AnPluginRegistry INSTANCE = new AnPluginRegistry();

    private final Map<String, AnPlugin> plugins = new HashMap<>();

    private Activity host;

    private AnPluginRegistry() {}

    /**
     * Se llama una vez, antes de crear el {@link AnRuntime}: el core construye un módulo por plugin
     * registrado al arrancar el motor, y lo que llegue después ya no entraría.
     */
    public static void install(Activity host) {
        INSTANCE.host = host;
        AnRuntime.setPluginRegistry(INSTANCE);
        AnGeneratedPlugins.install();
    }

    /**
     * La llama el fichero generado, una vez por plugin. El nombre viene del
     * {@code package.json} del plugin, que es el único sitio donde se escribe.
     */
    public static void register(String name, AnPlugin plugin) {
        if (INSTANCE.plugins.containsKey(name)) {
            Log.w(TAG, "dos plugins dicen llamarse " + name + "; se queda el primero");
            return;
        }
        plugin.attach(INSTANCE.host);
        INSTANCE.plugins.put(name, plugin);
        AnRuntime.registerPlugin(name);
    }

    /** Rust llama aquí desde el hilo de UI, dentro del frame. */
    public void dispatch(long id, String module, String method, String args) {
        AnPluginCall respond = new AnPluginCall(id);
        AnPlugin plugin = plugins.get(module);
        if (plugin == null) {
            // No debería poder pasar: el core solo conoce los nombres que este
            // registro le dio. Si pasa, se dice en vez de dejar la promesa
            // colgada.
            respond.reject("el plugin " + module + " no está en este APK");
            return;
        }
        JSONObject parsed;
        try {
            parsed = new JSONObject(args == null ? "{}" : args);
        } catch (Exception error) {
            // JS puede mandar cualquier cosa, no solo un objeto. Se entrega
            // vacío en vez de reventar: es lo mismo que hace el shell de iOS.
            parsed = new JSONObject();
        }
        try {
            plugin.call(method, parsed, respond);
        } catch (RuntimeException error) {
            // Una excepción del plugin no puede tumbar el frame, pero tampoco
            // puede desaparecer: se convierte en el rechazo de esa promesa.
            respond.reject(module + "." + method + " lanzó: " + error);
        }
    }
}
