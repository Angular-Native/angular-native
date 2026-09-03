package dev.angularnative;

import android.util.Log;

import org.json.JSONArray;
import org.json.JSONObject;

/**
 * The answer to a plugin call.
 *
 * <p>It can be answered from any thread: on the other side there is a locked mailbox, not a
 * variable of the UI thread. What cannot be done is not answering —the promise on the JS side is
 * left waiting for ever—, so every error path has to end in {@link #reject}.
 */
public final class AnPluginCall {

    private static final String TAG = "angular-native";

    private final long id;
    private boolean answered;

    AnPluginCall(long id) {
        this.id = id;
    }

    /** For a method that returns nothing. */
    public void resolve() {
        send("null");
    }

    public void resolve(String text) {
        // `JSONObject.quote` escapes and adds the quotes: it is the JSON escaping the platform
        // ships, without wrapping the string in an object.
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
        AnRuntime.pluginReject(id, message == null ? "the plugin failed" : message);
    }

    private void send(String json) {
        synchronized (this) {
            if (answered) {
                Log.w(TAG, "a plugin answered the same call twice");
                return;
            }
            answered = true;
        }
        AnRuntime.pluginResolve(id, json);
    }
}
