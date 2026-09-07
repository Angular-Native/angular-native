package dev.angularnative;

import android.app.Activity;
import android.content.Intent;
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

    /**
     * Which plugin is waiting for which request code.
     *
     * <p>The built-in modules have had this from the start; the plugins did not, so a plugin that
     * opened a camera or a picker had no way of hearing the answer. The codes start high enough
     * not to collide with the built-ins', which count up from zero.
     */
    private final Map<Integer, AnPlugin> byRequestCode = new HashMap<>();

    private int nextRequestCode = 9000;

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

    /**
     * Hands a plugin its own request code, so {@link #onActivityResult} can find it again.
     *
     * <p>Call it once, from {@code attach}: the number has to be stable for the life of the
     * process, because the Activity may be recreated while the chooser is on screen and the
     * result comes back to whatever is standing there afterwards.
     */
    public static int reserveRequestCode(AnPlugin plugin) {
        int code = INSTANCE.nextRequestCode++;
        INSTANCE.byRequestCode.put(code, plugin);
        return code;
    }

    /** Forwarded by {@link MainActivity}. Returns whether it belonged to a plugin. */
    public static boolean onActivityResult(int requestCode, int resultCode, Intent data) {
        AnPlugin plugin = INSTANCE.byRequestCode.get(requestCode);
        if (plugin == null) {
            return false;
        }
        plugin.onActivityResult(resultCode, data);
        return true;
    }

    /**
     * Forwarded by {@link MainActivity}. Every plugin hears it, because a permission dialog is
     * not addressed to one: two plugins can want the camera, and the answer is the same answer.
     */
    public static void onPermissionResult(String[] permissions, int[] granted) {
        for (AnPlugin plugin : INSTANCE.plugins.values()) {
            plugin.onPermissionResult(permissions, granted);
        }
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
