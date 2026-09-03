package dev.angularnative;

import android.app.Activity;
import android.util.Log;

import org.json.JSONObject;

import java.util.HashMap;
import java.util.Map;

/**
 * The APK's plugin registry.
 *
 * <p>Who is inside is decided by {@code an} when it builds the APK, reading the dependencies of the
 * app's {@code package.json}: it writes them into {@code AnGeneratedPlugins.java}, which is what
 * {@link #install(Activity)} calls.
 */
public final class AnPluginRegistry {

    private static final String TAG = "angular-native";

    private static final AnPluginRegistry INSTANCE = new AnPluginRegistry();

    private final Map<String, AnPlugin> plugins = new HashMap<>();

    private Activity host;

    private AnPluginRegistry() {}

    /**
     * Called once, before creating the {@link AnRuntime}: the core builds one module per registered
     * plugin when the engine starts, and anything arriving after that would no longer get in.
     */
    public static void install(Activity host) {
        INSTANCE.host = host;
        AnRuntime.setPluginRegistry(INSTANCE);
        AnGeneratedPlugins.install();
    }

    /**
     * Called by the generated file, once per plugin. The name comes from the plugin's
     * {@code package.json}, which is the only place it is written.
     */
    public static void register(String name, AnPlugin plugin) {
        if (INSTANCE.plugins.containsKey(name)) {
            Log.w(TAG, "two plugins claim to be called " + name + "; the first one stays");
            return;
        }
        plugin.attach(INSTANCE.host);
        INSTANCE.plugins.put(name, plugin);
        AnRuntime.registerPlugin(name);
    }

    /** Rust calls in here from the UI thread, inside the frame. */
    public void dispatch(long id, String module, String method, String args) {
        AnPluginCall respond = new AnPluginCall(id);
        AnPlugin plugin = plugins.get(module);
        if (plugin == null) {
            // This should not be able to happen: the core only knows the names this registry
            // gave it. If it does happen, it is said rather than leaving the promise hanging.
            respond.reject("the plugin " + module + " is not in this APK");
            return;
        }
        JSONObject parsed;
        try {
            parsed = new JSONObject(args == null ? "{}" : args);
        } catch (Exception error) {
            // JS can send anything, not only an object. It is handed over empty rather than
            // blowing up: it is the same thing the iOS shell does.
            parsed = new JSONObject();
        }
        try {
            plugin.call(method, parsed, respond);
        } catch (RuntimeException error) {
            // An exception from the plugin cannot bring the frame down, but it cannot disappear
            // either: it becomes the rejection of that promise.
            respond.reject(module + "." + method + " threw: " + error);
        }
    }
}
