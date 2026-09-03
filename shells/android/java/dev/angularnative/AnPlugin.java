package dev.angularnative;

import android.app.Activity;

import org.json.JSONObject;

/**
 * What an Android plugin implements.
 *
 * <p>It does not declare its own name: the name JS calls it by is in the {@code
 * angularNative.module} of its {@code package.json}, and that is where {@code an} takes it from
 * when generating the registry. One single place to write it is one fewer place where two copies
 * can stop matching.
 */
public interface AnPlugin {

    /**
     * The app's screen, before the first call.
     *
     * <p>Almost everything on Android asks for a context, and a plugin has nowhere to get one. It
     * is handed the Activity: both the context and the place to present something come out of it.
     * Whoever does not need it implements nothing.
     */
    default void attach(Activity host) {}

    /**
     * Handles one call.
     *
     * <p>{@code args} is what JS sent, already decoded; if it sent something that is not an object,
     * this arrives empty rather than null. It can be answered on the spot or the {@link
     * AnPluginCall} kept and answered later —that is what separates reading the clipboard from
     * taking a photo—, but it always has to be answered: a method that does not exist is rejected,
     * not ignored.
     */
    void call(String method, JSONObject args, AnPluginCall respond);
}
