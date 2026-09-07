package dev.angularnative;

import android.content.Context;
import android.util.Log;

import org.json.JSONObject;

import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;

/**
 * Which JavaScript the app runs, and what to do when a new one is wrong.
 *
 * <p>The APK always carries a {@code main.js} in its assets, and that one can never fail: it was
 * there when the app was reviewed and installed. An update is a second bundle written into the
 * app's own files directory, and the whole difficulty is that a bad one is only discovered
 * <em>after</em> it has been loaded — by which point, without care, the app cannot start well
 * enough to fix itself.
 *
 * <p>So an installed update is on probation. It is loaded once, marked {@code pending}; if the JS
 * reaches a point where it is plainly working and calls {@code updater.notifyReady()}, it becomes
 * {@code good}. If the app starts again and finds a bundle still {@code pending}, that bundle
 * crashed or hung before it could confirm, and it is thrown away.
 *
 * <p>That rule is the whole design. It costs one launch to recover from a broken update and needs
 * nothing from a server. The iOS half is {@code AnBundles.swift} and says the same thing.
 */
public final class AnBundles {

    private static final String TAG = "angular-native";

    private AnBundles() {}

    /** Where an update lives once installed. */
    public static File directory(Context context) {
        File directory = new File(context.getFilesDir(), "angular-native/bundles");
        //noinspection ResultOfMethodCallIgnored
        directory.mkdirs();
        return directory;
    }

    private static File installed(Context context) {
        return new File(directory(context), "main.js");
    }

    private static File state(Context context) {
        return new File(directory(context), "state.json");
    }

    /**
     * The source to evaluate at launch.
     *
     * <p>Called once, before anything else: it is also what retires a bundle that did not confirm
     * itself. {@code packaged} is what the caller passes in — the assets are the shell's to read,
     * not this class's.
     */
    public static String source(Context context, String packaged) {
        JSONObject details = read(context);
        if (details == null) {
            return packaged;
        }
        String status = details.optString("status", "");
        String version = details.optString("version", "");

        if ("pending".equals(status)) {
            // It was loaded once and never confirmed. That is what a bundle that
            // crashes on launch looks like from here, and one launch is all it
            // gets.
            Log.w(TAG, "update " + version + " did not confirm itself; going back to the packaged bundle");
            discard(context);
            return packaged;
        }

        String script = readFile(installed(context));
        if (script == null) {
            discard(context);
            return packaged;
        }
        if (!"good".equals(status)) {
            // Freshly installed and never yet loaded: this launch is its trial.
            write(context, version, "pending");
        }
        Log.i(TAG, "running update " + version);
        return script;
    }

    /** The JS says it is working. Called by the updater plugin. */
    public static void confirm(Context context) {
        JSONObject details = read(context);
        if (details == null || "good".equals(details.optString("status", ""))) {
            return;
        }
        write(context, details.optString("version", ""), "good");
    }

    /**
     * Installs a downloaded bundle. It is not loaded until the next launch: swapping the
     * JavaScript under a running app would leave the native views on screen belonging to a tree
     * nothing remembers building.
     */
    public static void install(Context context, File staged, String version) throws Exception {
        File destination = installed(context);
        //noinspection ResultOfMethodCallIgnored
        destination.delete();
        if (!staged.renameTo(destination)) {
            throw new IllegalStateException("the bundle could not be moved into place");
        }
        write(context, version, "fresh");
    }

    /** Throws the update away and goes back to what the APK carries. */
    public static void discard(Context context) {
        //noinspection ResultOfMethodCallIgnored
        installed(context).delete();
        //noinspection ResultOfMethodCallIgnored
        state(context).delete();
    }

    /** What is installed right now, whatever its state. */
    public static JSONObject current(Context context) {
        JSONObject details = read(context);
        JSONObject out = new JSONObject();
        try {
            out.put("version", details == null ? "packaged" : details.optString("version", "packaged"));
            out.put("status", details == null ? "packaged" : details.optString("status", "packaged"));
        } catch (Exception impossible) {
            // Two string keys and two string values.
        }
        return out;
    }

    private static JSONObject read(Context context) {
        String text = readFile(state(context));
        if (text == null) {
            return null;
        }
        try {
            return new JSONObject(text);
        } catch (Exception malformed) {
            return null;
        }
    }

    private static void write(Context context, String version, String status) {
        try {
            JSONObject details = new JSONObject();
            details.put("version", version);
            details.put("status", status);
            try (FileOutputStream out = new FileOutputStream(state(context))) {
                out.write(details.toString().getBytes(StandardCharsets.UTF_8));
            }
        } catch (Exception error) {
            Log.w(TAG, "the update state could not be written", error);
        }
    }

    private static String readFile(File file) {
        if (!file.isFile()) {
            return null;
        }
        try (InputStream in = new java.io.FileInputStream(file)) {
            byte[] bytes = new byte[(int) file.length()];
            int read = in.read(bytes);
            return read <= 0 ? null : new String(bytes, 0, read, StandardCharsets.UTF_8);
        } catch (Exception error) {
            return null;
        }
    }
}
