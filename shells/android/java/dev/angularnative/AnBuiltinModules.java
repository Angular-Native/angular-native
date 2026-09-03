package dev.angularnative;

import android.app.Activity;
import android.content.Intent;
import android.util.Log;

import org.json.JSONObject;

import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * The built-in modules of the Android shell.
 *
 * <p>It is the twin of {@link AnPluginRegistry} with one difference that matters: who is inside is
 * not read from anybody's {@code package.json}. These modules ship with the framework and every host
 * has them, so the list is written here and in {@code BUILTIN_MODULES} on the Rust side, and {@code
 * scripts/check-builtins.sh} reads both.
 *
 * <p>It also owns the request codes. A module that has to open a system chooser needs one, and two
 * modules picking the same number by hand would each get the other's answer.
 */
public final class AnBuiltinModules {

    private static final String TAG = "angular-native";

    /**
     * Where this shell's request codes start. Far enough from 0 that an app embedding the runtime
     * and starting activities of its own does not collide with it by accident.
     */
    private static final int FIRST_REQUEST_CODE = 0x616E00;

    private static final AnBuiltinModules INSTANCE = new AnBuiltinModules();

    private final Map<String, AnBuiltinModule> modules = new LinkedHashMap<>();
    private final Map<Integer, AnBuiltinModule> byRequestCode = new HashMap<>();

    private Activity host;
    private int nextRequestCode = FIRST_REQUEST_CODE;

    private AnBuiltinModules() {}

    /**
     * Called once, before creating the {@link AnRuntime}: the core builds one native module per name
     * when the engine starts, and anything arriving after that would no longer get in.
     */
    public static void install(Activity host) {
        INSTANCE.host = host;
        AnRuntime.setBuiltinModules(INSTANCE);
        if (!INSTANCE.modules.isEmpty()) {
            // The Activity was recreated —a rotation, a configuration change,
            // the app coming back from the background— and this registry is not:
            // it belongs to the process, like the plugins'. What has to be
            // renewed is the screen, because the one the modules were holding is
            // gone and presenting anything from it would do nothing at all.
            for (AnBuiltinModule module : INSTANCE.modules.values()) {
                module.attach(host);
            }
            return;
        }
        INSTANCE.register("files", new AnFiles());
        INSTANCE.register("share", new AnShare());
    }

    private void register(String name, AnBuiltinModule module) {
        if (modules.containsKey(name)) {
            Log.w(TAG, "the built-in module " + name + " was installed twice");
            return;
        }
        modules.put(name, module);
        module.attach(host);
    }

    /**
     * Hands a module its own request code, so that {@link #onActivityResult} can find it again. It
     * is called from {@code attach} and the number never changes for the life of the process.
     */
    public static int reserveRequestCode(AnBuiltinModule module) {
        int code = INSTANCE.nextRequestCode++;
        INSTANCE.byRequestCode.put(code, module);
        return code;
    }

    /** Rust calls in here from the UI thread, inside the frame. */
    public void dispatch(long id, String module, String method, String args) {
        AnBuiltinCall respond = new AnBuiltinCall(id);
        AnBuiltinModule implementation = modules.get(module);
        if (implementation == null) {
            // This should not be able to happen: the names come from BUILTIN_MODULES and they
            // are the same ones registered above. If it does happen it is said, rather than
            // leaving the promise hanging.
            respond.reject(
                    "the built-in module "
                            + module
                            + " is not installed in this shell; it is in BUILTIN_MODULES and"
                            + " nobody registered it");
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
            implementation.call(method, parsed, respond);
        } catch (RuntimeException error) {
            // An exception from a module cannot bring the frame down, but it cannot disappear
            // either: it becomes the rejection of that promise.
            respond.reject(module + "." + method + " threw: " + error);
        }
    }

    /** Forwarded by {@link MainActivity}. Returns whether it belonged to a built-in module. */
    public static boolean onActivityResult(int requestCode, int resultCode, Intent data) {
        AnBuiltinModule module = INSTANCE.byRequestCode.get(requestCode);
        if (module == null) {
            return false;
        }
        module.onActivityResult(resultCode, data);
        return true;
    }
}
