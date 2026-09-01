package dev.angularnative;

import android.app.Activity;
import android.graphics.Color;
import android.os.Bundle;
import android.util.DisplayMetrics;
import android.util.Log;
import android.view.Choreographer;
import android.view.ViewGroup;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;

/**
 * Todo el shell de Android cabe aquí, igual que el de iOS: crear el runtime,
 * darle una vista donde montar, avisarle del tamaño y llamarle una vez por
 * frame.
 */
public final class MainActivity extends androidx.appcompat.app.AppCompatActivity {

    static {
        // Los componentes de Material siguen al sistema, y las apps de aquí
        // pintan sus colores a mano: con el sistema en claro salía una barra
        // de navegación blanca debajo de una pantalla oscura.
        //
        // Se fuerza el oscuro hasta que la apariencia sea algo que la app
        // declare. Debería serlo: es una decisión suya, no del shell.
        androidx.appcompat.app.AppCompatDelegate.setDefaultNightMode(
                androidx.appcompat.app.AppCompatDelegate.MODE_NIGHT_YES);
    }

    private static final String TAG = "angular-native";

    private AnRuntime runtime;
    private AnHost host;
    private Choreographer.FrameCallback frameCallback;
    private DevClient devClient;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);

        AnViewGroup container = new AnViewGroup(this);
        container.setBackgroundColor(Color.BLACK);
        setContentView(
                container,
                new ViewGroup.LayoutParams(
                        ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));

        DisplayMetrics metrics = getResources().getDisplayMetrics();
        float widthDp = metrics.widthPixels / metrics.density;
        float heightDp = metrics.heightPixels / metrics.density;

        // Antes de crear el runtime: el core construye un módulo nativo por
        // plugin al arrancar el motor, y lo que se registre después no entra.
        AnPluginRegistry.install(this);

        host = new AnHost(this, container);
        runtime = new AnRuntime(host, widthDp, heightDp);
        if (!runtime.isValid()) {
            Log.e(TAG, "el runtime no arrancó");
            return;
        }
        host.attachRuntime(runtime);

        String source = readAsset("main.js");
        if (source == null) {
            Log.e(TAG, "no hay main.js en los assets");
        } else if (runtime.eval("main.js", source) != 0) {
            Log.e(TAG, "main.js lanzó al evaluarse");
        }

        // Solo existe si el APK lo armó `an dev`.
        devClient =
                DevClient.create(
                        readAsset("dev-server.txt"),
                        code -> {
                            Log.i(TAG, "recargando");
                            if (runtime.reload("main.js", code) != 0) {
                                Log.e(TAG, "el bundle recargado lanzó al evaluarse");
                            }
                        });
        if (devClient != null) {
            devClient.start();
        }

        // El reloj de la app es el del vsync, igual que el CADisplayLink de
        // iOS: los temporizadores de JS avanzan con los frames.
        frameCallback =
                new Choreographer.FrameCallback() {
                    @Override
                    public void doFrame(long frameTimeNanos) {
                        int applied = runtime.frame(frameTimeNanos / 1_000_000.0);
                        if (applied < 0) {
                            Log.e(TAG, "el frame falló");
                        } else if (applied > 0) {
                            host.flush();
                        }
                        Choreographer.getInstance().postFrameCallback(this);
                    }
                };
        Choreographer.getInstance().postFrameCallback(frameCallback);
    }

    @Override
    @SuppressWarnings("deprecation")
    public void onBackPressed() {
        // Si la app tiene una pila con pantallas encima, atrás navega dentro.
        // Si no, se comporta como siempre y sale.
        if (host == null || !host.dispatchBack()) {
            super.onBackPressed();
        }
    }

    @Override
    protected void onDestroy() {
        if (frameCallback != null) {
            Choreographer.getInstance().removeFrameCallback(frameCallback);
        }
        if (devClient != null) {
            devClient.stop();
        }
        if (runtime != null) {
            runtime.close();
        }
        super.onDestroy();
    }

    private String readAsset(String name) {
        try (InputStream input = getAssets().open(name)) {
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] chunk = new byte[8192];
            int read;
            while ((read = input.read(chunk)) != -1) {
                out.write(chunk, 0, read);
            }
            return out.toString(StandardCharsets.UTF_8.name());
        } catch (IOException error) {
            return null;
        }
    }
}
