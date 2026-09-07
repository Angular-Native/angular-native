package dev.angularnative;

import org.json.JSONArray;
import org.json.JSONObject;

/**
 * How a plugin says something nobody asked for.
 *
 * <p>An {@link AnPluginCall} answers one call, once. This is the other road: a position while
 * walking, a notification being tapped, a socket's messages. None of those is the answer to
 * anything, and a promise cannot carry them.
 *
 * <p>It can be called from any thread — a location callback, a service, a worker. The event is put
 * in a mailbox and delivered to JS at the top of the next frame, in the order it was emitted.
 *
 * <p>The module name is the one in the plugin's {@code angularNative.module}. Emitting under a name
 * nobody registered is logged rather than dropped in silence: it is a typo, and typos that vanish
 * are the expensive kind.
 *
 * <pre>{@code
 * JSONObject where = new JSONObject();
 * where.put("latitude", fix.getLatitude());
 * AnEvents.emit("geolocation", "position", where);
 * }</pre>
 */
public final class AnEvents {

    private AnEvents() {}

    /** An event with an object payload. */
    public static void emit(String module, String event, JSONObject payload) {
        AnRuntime.pluginEmit(module, event, payload == null ? "null" : payload.toString());
    }

    /** An event with a list payload. */
    public static void emit(String module, String event, JSONArray payload) {
        AnRuntime.pluginEmit(module, event, payload == null ? "null" : payload.toString());
    }

    /** An event with a string payload. */
    public static void emit(String module, String event, String payload) {
        AnRuntime.pluginEmit(module, event, payload == null ? "null" : JSONObject.quote(payload));
    }

    /** An event that carries nothing but its own name. */
    public static void emit(String module, String event) {
        AnRuntime.pluginEmit(module, event, "null");
    }
}
