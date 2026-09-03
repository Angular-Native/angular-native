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
 * The whole Android shell fits in here, just like the iOS one: create the
 * runtime, give it a view to mount into, tell it the size and call it once per
 * frame.
 */
public final class MainActivity extends androidx.appcompat.app.AppCompatActivity {

    static {
        // The Material components follow the system, and the apps here paint
        // their colours by hand: with the system in light mode a white
        // navigation bar came out underneath a dark screen.
        //
        // Dark is forced until the appearance is something the app declares. It
        // should be: it is the app's decision, not the shell's.
        androidx.appcompat.app.AppCompatDelegate.setDefaultNightMode(
                androidx.appcompat.app.AppCompatDelegate.MODE_NIGHT_YES);
    }

    private static final String TAG = "angular-native";

    private AnRuntime runtime;
    /** The viewport the engine has been told about, so it is only told changes. */
    private float viewportWidthDp;
    private float viewportHeightDp;
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

        // The screen's size, which is a starting guess and not the answer: the
        // engine has to be given a viewport before the first frame, and at this
        // point the container has not been laid out. What the app really gets
        // is settled below, when it is.
        DisplayMetrics metrics = getResources().getDisplayMetrics();
        float widthDp = metrics.widthPixels / metrics.density;
        float heightDp = metrics.heightPixels / metrics.density;
        viewportWidthDp = widthDp;
        viewportHeightDp = heightDp;

        // Before creating the runtime: the core builds one native module per
        // plugin when the engine starts, and whatever is registered afterwards
        // does not get in.
        AnPluginRegistry.install(this);

        host = new AnHost(this, container);
        runtime = new AnRuntime(host, widthDp, heightDp);
        if (!runtime.isValid()) {
            Log.e(TAG, "the runtime did not start");
            return;
        }
        host.attachRuntime(runtime);

        // The viewport follows the container, and not the screen.
        //
        // They are not the same number: a rotation swaps them, split screen and
        // a foldable change them without the app being recreated, and on the
        // first cold start the container can be laid out at a size the metrics
        // did not predict —that was a real phone showing the whole layout
        // squeezed into a square, portrait width by portrait width, while the
        // rest of the screen stayed black. Nothing called `setViewport`, so
        // taffy went on laying out for the size it was handed at startup for as
        // long as the app lived.
        container.addOnLayoutChangeListener((v, left, top, right, bottom, ol, ot, or_, ob) -> {
            float density = getResources().getDisplayMetrics().density;
            float w = (right - left) / density;
            float h = (bottom - top) / density;
            if (w == viewportWidthDp && h == viewportHeightDp) {
                return;
            }
            viewportWidthDp = w;
            viewportHeightDp = h;
            runtime.setViewport(w, h);
        });

        String source = readAsset("main.js");
        if (source == null) {
            Log.e(TAG, "there is no main.js in the assets");
        } else if (runtime.eval("main.js", source) != 0) {
            Log.e(TAG, "main.js threw while being evaluated");
        }

        // Only exists if the APK was built by `an dev`.
        devClient =
                DevClient.create(
                        readAsset("dev-server.txt"),
                        code -> {
                            Log.i(TAG, "reloading");
                            if (runtime.reload("main.js", code) != 0) {
                                Log.e(TAG, "the reloaded bundle threw while being evaluated");
                            }
                        });
        if (devClient != null) {
            devClient.start();
        }

        // The app's clock is the vsync one, just like the iOS CADisplayLink:
        // the JS timers advance with the frames.
        frameCallback =
                new Choreographer.FrameCallback() {
                    @Override
                    public void doFrame(long frameTimeNanos) {
                        int applied = runtime.frame(frameTimeNanos / 1_000_000.0);
                        if (applied < 0) {
                            Log.e(TAG, "the frame failed");
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
        // If the app has a stack with screens on it, back navigates inside it.
        // If not, it behaves as always and leaves.
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
