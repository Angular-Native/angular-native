package dev.angularnative;

import android.app.Activity;

import org.json.JSONObject;

/**
 * What a built-in module implements.
 *
 * <p>The name it answers to is in {@link AnBuiltinModules#install(Activity)}, and in
 * {@code BUILTIN_MODULES} on the Rust side. Unlike a plugin, nobody declares it in a
 * {@code package.json}: these ship with the framework and every host has them.
 */
public interface AnBuiltinModule {

    /**
     * The app's screen, before the first call. Almost everything on Android asks for a context, and
     * a module has nowhere else to get one.
     */
    default void attach(Activity host) {}

    /**
     * Handles one call.
     *
     * <p>{@code args} is what JS sent, already decoded; if it sent something that is not an object,
     * this arrives empty rather than null. It can be answered on the spot or the {@link
     * AnBuiltinCall} kept and answered later —that is what separates reading the network monitor
     * from putting a chooser on the screen—, but it always has to be answered.
     */
    void call(String method, JSONObject args, AnBuiltinCall respond);

    /**
     * A result from an activity this module started. It is only reached if the module asked for the
     * request code through {@link AnBuiltinModules#reserveRequestCode(AnBuiltinModule)}.
     */
    default void onActivityResult(int resultCode, android.content.Intent data) {}
}
