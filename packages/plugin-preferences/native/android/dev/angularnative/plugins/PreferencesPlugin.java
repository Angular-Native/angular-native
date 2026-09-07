package dev.angularnative.plugins;

import android.app.Activity;
import android.content.Context;
import android.content.SharedPreferences;

import dev.angularnative.AnPlugin;
import dev.angularnative.AnPluginCall;

import org.json.JSONArray;
import org.json.JSONObject;

import java.util.Map;

/**
 * Preferences on Android.
 *
 * <p>{@code SharedPreferences} under a file of this plugin's own rather than the default one. The
 * default file is shared with anything else in the process that asks for it — a library caching a
 * flag, the app's own settings screen — and writing there would mean {@code keys()} returning
 * things this app never wrote through here and {@code clear()} deleting them.
 *
 * <p>Everything is committed with {@code apply()} and not {@code commit()}: {@code apply} writes to
 * memory immediately and to disk on a background thread, and the in-memory value is what any later
 * read sees. A {@code commit()} would block the UI thread on a disk write to gain a guarantee this
 * API never promised.
 */
public final class PreferencesPlugin implements AnPlugin {

    private static final String STORE = "dev.angularnative.preferences";

    private Activity host;

    @Override
    public void attach(Activity host) {
        this.host = host;
    }

    @Override
    public void call(String method, JSONObject args, AnPluginCall respond) {
        if (host == null) {
            respond.reject("the preferences plugin has no Android context");
            return;
        }
        SharedPreferences store = host.getSharedPreferences(STORE, Context.MODE_PRIVATE);

        switch (method) {
            case "get": {
                String key = key(args, "get", respond);
                if (key == null) {
                    return;
                }
                // `getString` throws if something non-string was written under
                // the key. Nothing here writes anything else, and a plugin that
                // dies rather than answering leaves a promise hanging for ever.
                try {
                    respond.resolve(store.getString(key, null));
                } catch (ClassCastException wrongType) {
                    respond.resolve((String) null);
                }
                return;
            }

            case "set": {
                String key = key(args, "set", respond);
                if (key == null) {
                    return;
                }
                if (!args.has("value") || args.isNull("value")) {
                    respond.reject("preferences.set needs a string in 'value'");
                    return;
                }
                store.edit().putString(key, args.optString("value")).apply();
                respond.resolve();
                return;
            }

            case "remove": {
                String key = key(args, "remove", respond);
                if (key == null) {
                    return;
                }
                // Removing something that was never there is not an error: the
                // caller wanted it gone, and it is gone.
                store.edit().remove(key).apply();
                respond.resolve();
                return;
            }

            case "keys": {
                JSONArray keys = new JSONArray();
                for (Map.Entry<String, ?> entry : store.getAll().entrySet()) {
                    keys.put(entry.getKey());
                }
                respond.resolve(keys);
                return;
            }

            case "clear":
                store.edit().clear().apply();
                respond.resolve();
                return;

            default:
                respond.reject("the preferences plugin has no method " + method);
        }
    }

    /** The key, or null having already rejected — so the caller returns. */
    private static String key(JSONObject args, String method, AnPluginCall respond) {
        String key = args.optString("key", "");
        if (key.isEmpty()) {
            respond.reject("preferences." + method + " needs a key in 'key'");
            return null;
        }
        return key;
    }
}
