package dev.angularnative.plugins;

import android.app.Activity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;

import dev.angularnative.AnPlugin;
import dev.angularnative.AnPluginCall;

import org.json.JSONObject;

/**
 * The Android clipboard.
 *
 * <p>It is compiled into the APK in the same {@code javac} invocation as the shell, so it sees
 * {@link AnPlugin} and {@link AnPluginCall} with no extra classpath. What registers it is the file
 * {@code an} generates out of the {@code package.json}.
 *
 * <p>{@code ClipboardManager} wants the UI thread, and we are already on it here: the core delivers
 * the calls inside the frame. That is why the answer can be given on the spot instead of holding on
 * to the {@link AnPluginCall} for later.
 */
public final class ClipboardPlugin implements AnPlugin {

    /** The app's screen. An Android plugin nearly always needs a context. */
    private Activity host;

    @Override
    public void attach(Activity host) {
        this.host = host;
    }

    @Override
    public void call(String method, JSONObject args, AnPluginCall respond) {
        ClipboardManager clipboard =
                host == null
                        ? null
                        : (ClipboardManager) host.getSystemService(Context.CLIPBOARD_SERVICE);
        if (clipboard == null) {
            respond.reject("the clipboard plugin has no Android context");
            return;
        }

        switch (method) {
            case "write":
                String text = args.optString("text", null);
                if (text == null) {
                    respond.reject("clipboard.write needs a piece of text in 'text'");
                    return;
                }
                clipboard.setPrimaryClip(ClipData.newPlainText("angular-native", text));
                respond.resolve();
                return;

            case "read":
                // An empty string and not null: whoever asks for the clipboard
                // wants to paint something, and `undefined` would force a check
                // at every single use.
                respond.resolve(readText(clipboard));
                return;

            case "hasText":
                respond.resolve(clipboard.hasPrimaryClip() && !readText(clipboard).isEmpty());
                return;

            default:
                // Never in silence: a method that does not exist rejects the
                // promise saying which one was asked for.
                respond.reject("the clipboard plugin has no method " + method);
        }
    }

    private static String readText(ClipboardManager clipboard) {
        ClipData clip = clipboard.getPrimaryClip();
        if (clip == null || clip.getItemCount() == 0) {
            return "";
        }
        CharSequence text = clip.getItemAt(0).getText();
        return text == null ? "" : text.toString();
    }
}
