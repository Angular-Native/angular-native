package dev.angularnative;

import android.os.Handler;
import android.os.Looper;
import android.util.Log;

import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;

/**
 * Client for the development server.
 *
 * Android ships no WebSocket client in the platform, and adding OkHttp just for
 * this does not pay off: long polling over HTTP is used instead. The thread asks
 * `/wait`, the server does not answer until there is a reload, and then the
 * bundle is downloaded. It costs the same as a WebSocket and adds no
 * dependencies.
 *
 * The address is `127.0.0.1` and it is the device's own: `an dev` opens the port
 * back towards the machine serving the bundle with `adb reverse` before it
 * installs. That is why there is no emulator translation here any more, and why
 * a phone plugged in over USB reaches the server exactly as an emulator does.
 */
final class DevClient {

    private static final String TAG = "angular-native";

    private final String baseUrl;
    private final Reload onReload;
    private final Handler main = new Handler(Looper.getMainLooper());
    private volatile boolean running = true;

    interface Reload {
        void apply(String source);
    }

    private DevClient(String baseUrl, Reload onReload) {
        this.baseUrl = baseUrl;
        this.onReload = onReload;
    }

    /** Returns null if the APK was not built by `an dev`. */
    static DevClient create(String baseUrl, Reload onReload) {
        if (baseUrl == null || baseUrl.isEmpty()) {
            return null;
        }
        return new DevClient(baseUrl.trim(), onReload);
    }

    void start() {
        Thread thread =
                new Thread(
                        () -> {
                            Log.i(TAG, "connected to the development server at " + baseUrl);
                            while (running) {
                                try {
                                    if (waitForChange()) {
                                        String source = fetch("/bundle.js");
                                        if (source != null) {
                                            main.post(() -> onReload.apply(source));
                                        }
                                    }
                                } catch (Exception error) {
                                    Log.w(TAG, "development server unreachable: " + error);
                                    // Without a pause, a server that is down
                                    // would turn this into a full-speed loop.
                                    sleep(2000);
                                }
                            }
                        },
                        "an-dev");
        thread.setDaemon(true);
        thread.start();
    }

    void stop() {
        running = false;
    }

    private boolean waitForChange() throws Exception {
        HttpURLConnection connection = open("/wait");
        // Longer than the server's timeout, which answers 204 after 30 seconds.
        connection.setReadTimeout(45000);
        try {
            return connection.getResponseCode() == 200;
        } finally {
            connection.disconnect();
        }
    }

    private String fetch(String path) throws Exception {
        HttpURLConnection connection = open(path);
        try (InputStream input = connection.getInputStream()) {
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] chunk = new byte[8192];
            int read;
            while ((read = input.read(chunk)) != -1) {
                out.write(chunk, 0, read);
            }
            return out.toString(StandardCharsets.UTF_8.name());
        } finally {
            connection.disconnect();
        }
    }

    private HttpURLConnection open(String path) throws Exception {
        HttpURLConnection connection = (HttpURLConnection) new URL(baseUrl + path).openConnection();
        connection.setConnectTimeout(3000);
        connection.setReadTimeout(10000);
        return connection;
    }

    private static void sleep(long millis) {
        try {
            Thread.sleep(millis);
        } catch (InterruptedException ignored) {
            Thread.currentThread().interrupt();
        }
    }
}
