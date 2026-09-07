package dev.angularnative;

import android.content.Context;
import android.util.Log;
import android.view.View;

import java.util.HashMap;
import java.util.Map;

/**
 * Views a plugin brings.
 *
 * <p>A plugin contributes methods — that is the rule, and it is why a barcode scanner could only
 * ever be full screen. This is the one exception, and it is deliberately narrow: a plugin registers
 * a factory under a name, and a template mounts it with {@code <an-custom [view]="that name">}.
 *
 * <p>What it is <b>not</b> is a way to add a primitive. A primitive is a control the framework
 * mounts on every host, with a name every layer agrees about and a check that keeps them agreeing.
 * A plugin view is one platform's view, mounted where the template asked, sized by the layout and
 * nothing else.
 */
public final class AnPluginViews {

    private static final String TAG = "angular-native";

    /** A factory a plugin registers. It is handed the Activity as its context. */
    public interface Factory {
        View create(Context context);
    }

    private static final Map<String, Factory> FACTORIES = new HashMap<>();

    private AnPluginViews() {}

    /** Called by a plugin, from {@code attach}. */
    public static void register(String name, Factory factory) {
        if (FACTORIES.containsKey(name)) {
            Log.w(TAG, "two plugins register a view called " + name + "; the first one stays");
            return;
        }
        FACTORIES.put(name, factory);
    }

    /** The view registered under that name, or null — the host says which name was missing. */
    static View create(Context context, String name) {
        Factory factory = FACTORIES.get(name);
        return factory == null ? null : factory.create(context);
    }
}
