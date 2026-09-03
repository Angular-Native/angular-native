package dev.angularnative;

import android.util.Log;

import org.json.JSONArray;
import org.json.JSONObject;

/**
 * The answer to a built-in module call.
 *
 * <p>It is {@link AnPluginCall} under another name and over another mailbox, and it is separate for
 * the same reason the mailboxes are: a built-in cannot be shadowed by an npm package claiming its
 * name, and the two answer through different pairs of native methods.
 *
 * <p>It can be answered from any thread: on the other side there is a locked mailbox, not a variable
 * of the UI thread. What cannot be done is not answering —the promise on the JS side is left waiting
 * for ever—, so every error path has to end in {@link #reject}.
 */
public final class AnBuiltinCall {

    private static final String TAG = "angular-native";

    private final long id;
    private boolean answered;

    AnBuiltinCall(long id) {
        this.id = id;
    }

    /** For a method that returns nothing. */
    public void resolve() {
        send("null");
    }

    public void resolve(String text) {
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
        AnRuntime.builtinReject(id, message == null ? "the built-in module failed" : message);
    }

    private void send(String json) {
        synchronized (this) {
            if (answered) {
                Log.w(TAG, "a built-in module answered the same call twice");
                return;
            }
            answered = true;
        }
        AnRuntime.builtinResolve(id, json);
    }
}
