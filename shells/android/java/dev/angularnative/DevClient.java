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
 * Cliente del servidor de desarrollo.
 *
 * Android no trae cliente de WebSocket en la plataforma, y añadir OkHttp solo
 * para esto no compensa: se usa espera larga sobre HTTP. El hilo pregunta a
 * `/wait`, el servidor no contesta hasta que hay una recarga, y entonces se
 * baja el bundle. Cuesta lo mismo que un WebSocket y no añade dependencias.
 *
 * Desde el emulador, la máquina anfitriona es 10.0.2.2.
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

    /** Devuelve null si el APK no lo armó `an dev`. */
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
                            Log.i(TAG, "conectado al servidor de desarrollo en " + baseUrl);
                            while (running) {
                                try {
                                    if (waitForChange()) {
                                        String source = fetch("/bundle.js");
                                        if (source != null) {
                                            main.post(() -> onReload.apply(source));
                                        }
                                    }
                                } catch (Exception error) {
                                    Log.w(TAG, "servidor de desarrollo inalcanzable: " + error);
                                    // Sin pausa, un servidor caído convertiría
                                    // esto en un bucle a toda velocidad.
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
        // Más que el timeout del servidor, que responde 204 a los 30 segundos.
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
