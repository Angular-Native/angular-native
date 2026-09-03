package dev.angularnative;

import android.app.Activity;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.net.Uri;
import android.provider.OpenableColumns;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.HashMap;
import java.util.Map;

/**
 * The {@code files} module on Android.
 *
 * <p>Two things, and they are not the same thing:
 *
 * <ul>
 *   <li><b>The app's own storage.</b> {@code getFilesDir()} and {@code getCacheDir()}, which are
 *       inside the app's private directory. Nothing here asks for a permission, because since
 *       Android 10 an app does not need one to write in its own container — and it is exactly why
 *       this module does not offer the shared storage, which does.
 *   <li><b>What the person chose.</b> The Storage Access Framework —{@code
 *       Intent.ACTION_OPEN_DOCUMENT}— which is the system's own picker and the only way to reach a
 *       document outside the container.
 * </ul>
 *
 * <p>Between the two there is the same rule as on Apple's platforms: <b>it writes only inside the
 * app's directories, and outside them it reads only what the person picked, and only while the app
 * is running.</b> A path that is neither is turned down saying which of the two it failed.
 *
 * <p>What a picked file answers as its path is a {@code content://} URI and not a file path: that is
 * what the Storage Access Framework hands over, the file behind it may not be on this device at all,
 * and inventing a {@code /}-path for it would be inventing something the system never said. It is
 * read back through the {@link android.content.ContentResolver}.
 */
public final class AnFiles implements AnBuiltinModule {

    private Activity host;
    private int requestCode;

    /** What the person picked, by the URI handed back to JS. */
    private final Map<String, Uri> granted = new HashMap<>();

    /** The call waiting for the picker. Only one can be open at a time. */
    private AnBuiltinCall picking;

    @Override
    public void attach(Activity host) {
        this.host = host;
        // Reserved once and kept: `attach` comes round again whenever the
        // Activity is recreated, and a module that took a new code each time
        // would stop recognising the result of a picker it had already opened.
        if (requestCode == 0) {
            requestCode = AnBuiltinModules.reserveRequestCode(this);
        }
    }

    @Override
    public void call(String method, JSONObject args, AnBuiltinCall respond) {
        if (host == null) {
            respond.reject("the files module has no Android context");
            return;
        }
        switch (method) {
            case "documentsDirectory":
                respond.resolve(host.getFilesDir().getAbsolutePath());
                return;

            case "cacheDirectory":
                respond.resolve(host.getCacheDir().getAbsolutePath());
                return;

            case "exists":
                {
                    String asked = args.optString("path", "");
                    if (granted.containsKey(asked)) {
                        respond.resolve(true);
                        return;
                    }
                    File file = resolve(asked, respond);
                    if (file != null) {
                        respond.resolve(file.exists());
                    }
                    return;
                }

            case "read":
                read(args.optString("path", ""), respond);
                return;

            case "write":
                {
                    if (!args.has("text")) {
                        respond.reject("files.write needs the contents in 'text'");
                        return;
                    }
                    String text = args.optString("text", "");
                    File file = writable(args.optString("path", ""), respond);
                    if (file == null) {
                        return;
                    }
                    File parent = file.getParentFile();
                    if (parent != null && !parent.exists() && !parent.mkdirs()) {
                        respond.reject("files.write could not create " + parent.getAbsolutePath());
                        return;
                    }
                    try (FileOutputStream out = new FileOutputStream(file)) {
                        out.write(text.getBytes(StandardCharsets.UTF_8));
                        respond.resolve();
                    } catch (IOException error) {
                        respond.reject(
                                "files.write could not write "
                                        + file.getAbsolutePath()
                                        + ": "
                                        + error);
                    }
                    return;
                }

            case "remove":
                {
                    File file = writable(args.optString("path", ""), respond);
                    if (file == null) {
                        return;
                    }
                    if (!file.exists()) {
                        respond.reject("files.remove: there is nothing at " + file.getAbsolutePath());
                    } else if (deleteTree(file)) {
                        respond.resolve();
                    } else {
                        respond.reject("files.remove could not delete " + file.getAbsolutePath());
                    }
                    return;
                }

            case "makeDirectory":
                {
                    File file = writable(args.optString("path", ""), respond);
                    if (file == null) {
                        return;
                    }
                    if (file.isDirectory() || file.mkdirs()) {
                        respond.resolve();
                    } else {
                        respond.reject("files.makeDirectory could not create " + file.getAbsolutePath());
                    }
                    return;
                }

            case "list":
                {
                    File directory = resolve(args.optString("path", ""), respond);
                    if (directory == null) {
                        return;
                    }
                    File[] children = directory.listFiles();
                    if (children == null) {
                        respond.reject(
                                "files.list could not read " + directory.getAbsolutePath());
                        return;
                    }
                    Arrays.sort(children);
                    JSONArray entries = new JSONArray();
                    for (File child : children) {
                        entries.put(entry(child));
                    }
                    respond.resolve(entries);
                    return;
                }

            case "pick":
                pick(args, respond);
                return;

            default:
                respond.reject("the files module has no method " + method);
        }
    }

    // ── The rule ───────────────────────────────────────────────────────────

    /** A path that may be read: inside the app's directories. */
    private File resolve(String asked, AnBuiltinCall respond) {
        if (asked == null || asked.isEmpty()) {
            respond.reject("this files method needs a path in 'path'");
            return null;
        }
        File file =
                asked.startsWith("/") ? new File(asked) : new File(host.getFilesDir(), asked);
        String path;
        try {
            path = file.getCanonicalPath();
        } catch (IOException error) {
            respond.reject("files could not make sense of the path " + asked + ": " + error);
            return null;
        }
        for (File root : new File[] {host.getFilesDir(), host.getCacheDir()}) {
            String prefix;
            try {
                prefix = root.getCanonicalPath();
            } catch (IOException error) {
                continue;
            }
            if (path.equals(prefix) || path.startsWith(prefix + "/")) {
                return file;
            }
        }
        respond.reject(
                "files will not touch "
                        + asked
                        + ": it is outside the app's own directories and it is not something the"
                        + " person picked. What the app owns is files.documentsDirectory() and"
                        + " files.cacheDirectory(); anything else has to come back from"
                        + " files.pick().");
        return null;
    }

    /**
     * A path that may be written: inside the app's directories and nowhere else. A picked document
     * is not one of them — the picker grants a read, and writing back over somebody's document
     * without their having asked for it is not something the framework will do behind a
     * {@code write}.
     */
    private File writable(String asked, AnBuiltinCall respond) {
        if (asked != null && granted.containsKey(asked)) {
            respond.reject(
                    "files cannot write to "
                            + asked
                            + ": the system picker grants a read of what the person chose, not"
                            + " permission to change it. Write inside files.documentsDirectory()"
                            + " instead.");
            return null;
        }
        return resolve(asked, respond);
    }

    private void read(String asked, AnBuiltinCall respond) {
        Uri picked = asked == null ? null : granted.get(asked);
        if (picked != null) {
            try (InputStream input = host.getContentResolver().openInputStream(picked)) {
                if (input == null) {
                    respond.reject("files.read: nothing answered for " + asked);
                    return;
                }
                respond.resolve(new String(drain(input), StandardCharsets.UTF_8));
            } catch (IOException | SecurityException error) {
                respond.reject("files.read could not read " + asked + ": " + error);
            }
            return;
        }
        File file = resolve(asked, respond);
        if (file == null) {
            return;
        }
        try (InputStream input = new java.io.FileInputStream(file)) {
            respond.resolve(new String(drain(input), StandardCharsets.UTF_8));
        } catch (IOException error) {
            respond.reject("files.read could not read " + file.getAbsolutePath() + ": " + error);
        }
    }

    private static byte[] drain(InputStream input) throws IOException {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        byte[] chunk = new byte[8192];
        int read;
        while ((read = input.read(chunk)) != -1) {
            out.write(chunk, 0, read);
        }
        return out.toByteArray();
    }

    private static boolean deleteTree(File file) {
        File[] children = file.listFiles();
        if (children != null) {
            for (File child : children) {
                if (!deleteTree(child)) {
                    return false;
                }
            }
        }
        return file.delete();
    }

    private static JSONObject entry(File file) {
        JSONObject entry = new JSONObject();
        try {
            entry.put("name", file.getName());
            entry.put("path", file.getAbsolutePath());
            entry.put("size", file.isDirectory() ? 0 : file.length());
            entry.put("isDirectory", file.isDirectory());
        } catch (org.json.JSONException error) {
            // Only thrown for a null key, and none of these is null.
        }
        return entry;
    }

    // ── The system picker ──────────────────────────────────────────────────

    private void pick(JSONObject args, AnBuiltinCall respond) {
        if (picking != null) {
            respond.reject("files.pick is already showing a picker");
            return;
        }
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.setType("*/*");
        JSONArray types = args.optJSONArray("types");
        if (types != null && types.length() > 0) {
            String[] mime = new String[types.length()];
            for (int at = 0; at < types.length(); at++) {
                mime[at] = types.optString(at, "*/*");
            }
            intent.setType(mime.length == 1 ? mime[0] : "*/*");
            if (mime.length > 1) {
                intent.putExtra(Intent.EXTRA_MIME_TYPES, mime);
            }
        }
        intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, args.optBoolean("multiple", false));

        PackageManager packages = host.getPackageManager();
        if (packages.resolveActivity(intent, 0) == null) {
            // Which is what happens on a watch: Wear OS ships no document
            // provider and no picker to browse one with. It is said by name
            // rather than the intent being fired into nothing.
            boolean watch = packages.hasSystemFeature(PackageManager.FEATURE_WATCH);
            respond.reject(
                    watch
                            ? "Wear OS has no file picker: the watch ships no document provider,"
                                    + " so ACTION_OPEN_DOCUMENT resolves to nothing. Read what the"
                                    + " app itself wrote with files.read, or have the phone pick it"
                                    + " and send it over."
                            : "no app on this device can open a document: ACTION_OPEN_DOCUMENT"
                                    + " resolves to nothing.");
            return;
        }

        picking = respond;
        try {
            host.startActivityForResult(intent, requestCode);
        } catch (RuntimeException error) {
            picking = null;
            respond.reject("files.pick could not open the picker: " + error);
        }
    }

    @Override
    public void onActivityResult(int resultCode, Intent data) {
        AnBuiltinCall respond = picking;
        picking = null;
        if (respond == null) {
            return;
        }
        if (resultCode != Activity.RESULT_OK || data == null) {
            // Not a failure: somebody backed out of the picker. An empty list is
            // the answer, and a `catch` firing here would make cancelling look
            // like something breaking.
            respond.resolve(new JSONArray());
            return;
        }
        JSONArray entries = new JSONArray();
        if (data.getClipData() != null) {
            for (int at = 0; at < data.getClipData().getItemCount(); at++) {
                entries.put(remember(data.getClipData().getItemAt(at).getUri()));
            }
        } else if (data.getData() != null) {
            entries.put(remember(data.getData()));
        }
        respond.resolve(entries);
    }

    /** Turns a picked URI into an entry and remembers it, so a later read is legal. */
    private JSONObject remember(Uri uri) {
        granted.put(uri.toString(), uri);
        JSONObject entry = new JSONObject();
        String name = uri.getLastPathSegment();
        long size = 0;
        try (Cursor cursor = host.getContentResolver().query(uri, null, null, null, null)) {
            if (cursor != null && cursor.moveToFirst()) {
                int nameAt = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME);
                int sizeAt = cursor.getColumnIndex(OpenableColumns.SIZE);
                if (nameAt >= 0 && !cursor.isNull(nameAt)) {
                    name = cursor.getString(nameAt);
                }
                if (sizeAt >= 0 && !cursor.isNull(sizeAt)) {
                    size = cursor.getLong(sizeAt);
                }
            }
        } catch (RuntimeException error) {
            // The provider may answer nothing at all. The URI is still usable;
            // only the name and the size are not known.
        }
        try {
            entry.put("name", name == null ? uri.toString() : name);
            entry.put("path", uri.toString());
            entry.put("size", size);
            entry.put("isDirectory", false);
        } catch (org.json.JSONException error) {
            // Only thrown for a null key, and none of these is null.
        }
        return entry;
    }
}
