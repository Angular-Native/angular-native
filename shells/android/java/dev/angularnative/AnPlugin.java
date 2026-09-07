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

    /**
     * The result of something this plugin started with {@code startActivityForResult}.
     *
     * <p>A plugin that opens a camera, a photo picker or a chooser cannot answer inside {@code
     * call}: the answer arrives here, on the Activity, some seconds later. Reserve a request code
     * with {@link AnPluginRegistry#reserveRequestCode} and the shell will route it back.
     *
     * <p>Whoever starts nothing implements nothing.
     */
    default void onActivityResult(int resultCode, android.content.Intent data) {}

    /**
     * The person's answer to a permission dialog this plugin asked for.
     *
     * <p>Android grants at run time through a dialog owned by the Activity, and its result comes
     * back here. Without this a plugin could show the dialog and never learn what was chosen,
     * which is why {@code geolocation.request()} used to return the standing state rather than
     * the answer.
     */
    default void onPermissionResult(String[] permissions, int[] granted) {}
}
