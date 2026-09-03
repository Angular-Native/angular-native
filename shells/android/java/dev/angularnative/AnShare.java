package dev.angularnative;

import android.app.Activity;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.net.Uri;

import androidx.core.content.FileProvider;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.File;
import java.util.ArrayList;

/**
 * The {@code share} module on Android and Wear OS.
 *
 * <p>It is {@code Intent.ACTION_SEND} inside {@code Intent.createChooser}, which is Android's own
 * sheet and not a dialog of ours that looks like one. What appears in it is every app that declared
 * it can receive this kind of thing, and a framework cannot reproduce that list.
 *
 * <p>A file is not sent as a {@code file://} path. Since Android 7 handing one to another app throws
 * {@code FileUriExposedException}, and the way round it is a {@code content://} URI from a {@link
 * FileProvider}, which is why the manifest declares one. If the app's own manifest has no such
 * provider — which is what happens to somebody who copied the shell and left it out — the call is
 * rejected with the four lines to paste, rather than crashing at the first share.
 */
public final class AnShare implements AnBuiltinModule {

    /** Must match {@code android:authorities} in the manifest, after the package name. */
    private static final String AUTHORITY_SUFFIX = ".anfiles";

    private Activity host;

    @Override
    public void attach(Activity host) {
        this.host = host;
    }

    @Override
    public void call(String method, JSONObject args, AnBuiltinCall respond) {
        if (host == null) {
            respond.reject("the share module has no Android context");
            return;
        }
        switch (method) {
            case "canShare":
                respond.resolve(chooserExists());
                return;

            case "share":
                share(args, respond);
                return;

            default:
                respond.reject("the share module has no method " + method);
        }
    }

    private boolean chooserExists() {
        Intent probe = new Intent(Intent.ACTION_SEND);
        probe.setType("text/plain");
        return host.getPackageManager().resolveActivity(probe, 0) != null;
    }

    private void share(JSONObject args, AnBuiltinCall respond) {
        String text = args.optString("text", "");
        String url = args.optString("url", "");
        String title = args.optString("title", "");
        JSONArray files = args.optJSONArray("files");

        StringBuilder body = new StringBuilder(text);
        if (!url.isEmpty()) {
            if (body.length() > 0) {
                body.append('\n');
            }
            body.append(url);
        }

        ArrayList<Uri> uris = new ArrayList<>();
        if (files != null) {
            for (int at = 0; at < files.length(); at++) {
                String path = files.optString(at, "");
                if (path.isEmpty()) {
                    continue;
                }
                if (path.startsWith("content://")) {
                    // Something that came back from files.pick(): it is already
                    // a URI another app can open, and wrapping it again would
                    // break it.
                    uris.add(Uri.parse(path));
                    continue;
                }
                try {
                    uris.add(
                            FileProvider.getUriForFile(
                                    host, host.getPackageName() + AUTHORITY_SUFFIX, new File(path)));
                } catch (IllegalArgumentException error) {
                    respond.reject(
                            "share cannot hand "
                                    + path
                                    + " to another app: "
                                    + error.getMessage()
                                    + ". Android will not let a file:// path cross to another app,"
                                    + " so this goes through a FileProvider. Your"
                                    + " AndroidManifest.xml needs, inside <application>:\n"
                                    + "  <provider"
                                    + " android:name=\"androidx.core.content.FileProvider\""
                                    + " android:authorities=\""
                                    + host.getPackageName()
                                    + AUTHORITY_SUFFIX
                                    + "\" android:exported=\"false\""
                                    + " android:grantUriPermissions=\"true\">\n"
                                    + "    <meta-data"
                                    + " android:name=\"android.support.FILE_PROVIDER_PATHS\""
                                    + " android:resource=\"@xml/an_file_paths\" />\n"
                                    + "  </provider>\n"
                                    + "and the file itself has to be inside"
                                    + " files.documentsDirectory() or files.cacheDirectory().");
                    return;
                }
            }
        }

        if (body.length() == 0 && uris.isEmpty()) {
            respond.reject(
                    "share.share was given nothing to share: it needs at least one of 'text',"
                            + " 'url' or 'files'");
            return;
        }

        Intent send;
        if (uris.size() > 1) {
            send = new Intent(Intent.ACTION_SEND_MULTIPLE);
            send.putParcelableArrayListExtra(Intent.EXTRA_STREAM, uris);
            send.setType("*/*");
        } else if (uris.size() == 1) {
            send = new Intent(Intent.ACTION_SEND);
            send.putExtra(Intent.EXTRA_STREAM, uris.get(0));
            String type = host.getContentResolver().getType(uris.get(0));
            send.setType(type == null ? "*/*" : type);
        } else {
            send = new Intent(Intent.ACTION_SEND);
            send.setType("text/plain");
        }
        if (body.length() > 0) {
            send.putExtra(Intent.EXTRA_TEXT, body.toString());
        }
        if (!title.isEmpty()) {
            send.putExtra(Intent.EXTRA_SUBJECT, title);
        }
        send.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);

        if (host.getPackageManager().resolveActivity(send, 0) == null) {
            // A watch is where this actually happens. Wear OS has no mail
            // client and no messaging app of its own; what a given watch can
            // receive is Bluetooth and whatever the manufacturer added, and on
            // some there is nothing at all. It is asked rather than assumed —
            // and when the answer is nothing, it is said by name rather than an
            // empty chooser being put on somebody's wrist.
            boolean watch =
                    host.getPackageManager().hasSystemFeature(PackageManager.FEATURE_WATCH);
            respond.reject(
                    watch
                            ? "no app on this watch can receive a share: nothing installed on it"
                                    + " declares ACTION_SEND, so the chooser would come up empty."
                                    + " Ask share.canShare() first — on a Wear watch the answer"
                                    + " depends on the watch. Send it to the phone and share it"
                                    + " from there."
                            : "no app on this device can receive a share: ACTION_SEND resolves to"
                                    + " nothing.");
            return;
        }

        try {
            Intent chooser = Intent.createChooser(send, title.isEmpty() ? null : title);
            host.startActivity(chooser);
            // Android's chooser gives no answer back before API 22 and only the
            // component that was picked after it, through a broadcast receiver
            // the app has to register. What is answered here is what is actually
            // known: the sheet went up. Claiming to know whether the person then
            // went through with it would be inventing it.
            respond.resolve(true);
        } catch (android.content.ActivityNotFoundException error) {
            respond.reject("no app on this device can receive a share: " + error);
        } catch (RuntimeException error) {
            respond.reject("share.share could not open the chooser: " + error);
        }
    }
}
