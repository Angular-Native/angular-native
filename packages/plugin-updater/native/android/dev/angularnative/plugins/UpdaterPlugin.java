package dev.angularnative.plugins;

import android.app.Activity;

import dev.angularnative.AnBundles;
import dev.angularnative.AnPlugin;
import dev.angularnative.AnPluginCall;

import org.json.JSONException;
import org.json.JSONObject;

import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.security.MessageDigest;

/**
 * Shipping new JavaScript without a Play release.
 *
 * <p>The download and the checksum live here; which bundle actually runs, and the rule that
 * retires one that never confirmed itself, live in {@link AnBundles} in the shell — that decision
 * has to be made before any plugin exists, at the moment the runtime is handed its source.
 *
 * <p>The download runs on a thread of its own. The plugin call arrives on the UI thread inside the
 * frame, and a network round trip there would stop the app for as long as it took.
 */
public final class UpdaterPlugin implements AnPlugin {

    private Activity host;

    @Override
    public void attach(Activity host) {
        this.host = host;
    }

    @Override
    public void call(String method, JSONObject args, AnPluginCall respond) {
        if (host == null) {
            respond.reject("the updater plugin has no Android context");
            return;
        }
        switch (method) {
            case "current":
                respond.resolve(AnBundles.current(host));
                return;

            case "notifyReady":
                // Doing nothing when the packaged bundle is running is
                // deliberate: an app should be able to call this at startup
                // without knowing what it is running.
                AnBundles.confirm(host);
                respond.resolve();
                return;

            case "reset":
                AnBundles.discard(host);
                respond.resolve();
                return;

            case "download":
                download(args, respond);
                return;

            default:
                respond.reject("the updater plugin has no method " + method);
        }
    }

    private void download(JSONObject args, AnPluginCall respond) {
        String address = args.optString("url", "");
        String version = args.optString("version", "");
        String expected = args.optString("sha256", "").toLowerCase(java.util.Locale.ROOT);
        if (address.isEmpty()) {
            respond.reject("updater.download needs a url in 'url'");
            return;
        }
        if (!address.startsWith("https://")) {
            // Not pedantry. This file becomes the code the app runs; over http
            // anyone on the path chooses what that code is.
            respond.reject("updater.download refuses that scheme: a bundle must come over https");
            return;
        }
        if (version.isEmpty()) {
            respond.reject("updater.download needs a version in 'version'");
            return;
        }

        new Thread(
                        () -> {
                            File staging =
                                    new File(
                                            AnBundles.directory(host),
                                            "staging-" + System.nanoTime() + ".js");
                            try {
                                HttpURLConnection connection =
                                        (HttpURLConnection) new URL(address).openConnection();
                                connection.setConnectTimeout(15000);
                                connection.setReadTimeout(30000);
                                int status = connection.getResponseCode();
                                if (status < 200 || status >= 300) {
                                    respond.reject("updater.download got HTTP " + status);
                                    return;
                                }
                                MessageDigest digest = MessageDigest.getInstance("SHA-256");
                                try (InputStream in = connection.getInputStream();
                                        FileOutputStream out = new FileOutputStream(staging)) {
                                    byte[] chunk = new byte[8192];
                                    int read;
                                    while ((read = in.read(chunk)) != -1) {
                                        digest.update(chunk, 0, read);
                                        out.write(chunk, 0, read);
                                    }
                                }
                                if (!expected.isEmpty()) {
                                    StringBuilder actual = new StringBuilder();
                                    for (byte b : digest.digest()) {
                                        actual.append(String.format("%02x", b));
                                    }
                                    if (!actual.toString().equals(expected)) {
                                        // Nothing is installed. A mismatch is a
                                        // corrupted download or somebody else's
                                        // file, and there is no version of this
                                        // where running it is right.
                                        //noinspection ResultOfMethodCallIgnored
                                        staging.delete();
                                        respond.reject(
                                                "updater.download: the bundle does not match its"
                                                        + " sha256 — expected "
                                                        + expected
                                                        + ", got "
                                                        + actual);
                                        return;
                                    }
                                }
                                AnBundles.install(host, staging, version);
                                respond.resolve();
                            } catch (Exception error) {
                                //noinspection ResultOfMethodCallIgnored
                                staging.delete();
                                respond.reject("updater.download failed: " + error);
                            }
                        },
                        "an-updater")
                .start();
    }
}
