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
        // The same library `AnRuntime` loads, and loading it twice is a no-op.
        // It is named here because the deep-link door below is this class's own
        // native method, and it may be knocked on before a runtime exists: on a
        // cold start the intent is read before the engine is built.
        System.loadLibrary("an_android");
    }

    private static final String TAG = "angular-native";

    /**
     * Hands a URL to the core. Global to the process and not to the runtime: a
     * link can be the reason the process exists, so it may be called before
     * there is anything to hand it to. What arrives early is kept and read by
     * the app as it boots; what arrives later reaches it as an event.
     *
     * <p>Implemented in `crates/an-android/src/jni_bridge.rs`.
     */
    private static native void nativeOpenUrl(String url);

    /**
     * The URL an intent is asking for, or null if it is not asking for one.
     *
     * <p>Only ACTION_VIEW: the launcher's own intent has no data, and a SEND
     * from another app is a share and not a route.
     */
    private static String linkOf(android.content.Intent intent) {
        if (intent == null || !android.content.Intent.ACTION_VIEW.equals(intent.getAction())) {
            return null;
        }
        android.net.Uri data = intent.getData();
        return data == null ? null : data.toString();
    }

    /** Where `an` leaves `app.appearance`. Written by `crates/an-cli/src/android.rs`. */
    private static final String APPEARANCE_KEY = "dev.angularnative.appearance";

    /**
     * What the app looks like, and it is the app that decides.
     *
     * <p>This used to be a static block forcing {@code MODE_NIGHT_YES} on every app built with
     * this shell. The reason was real — an example painting a dark background under a system in
     * light mode came out with a white navigation bar — but the fix took the choice away from
     * everybody to cure a symptom that belonged to the bars, not to the theme. The bars are
     * handled where they live now, in {@code AnHost}, and this reads what the app asked for.
     *
     * <p>It is what the Material components follow: dialogs, date pickers, the text selection
     * handles. It is not what paints the screen — that is the app's own background — so an app
     * that wants to follow the device has to paint with the device too, and one that pins itself
     * to {@code dark} should paint dark.
     */
    private void applyAppearance() {
        String appearance = "system";
        try {
            android.content.pm.ApplicationInfo info =
                    getPackageManager()
                            .getApplicationInfo(getPackageName(),
                                    android.content.pm.PackageManager.GET_META_DATA);
            if (info.metaData != null) {
                appearance = info.metaData.getString(APPEARANCE_KEY, "system");
            }
        } catch (android.content.pm.PackageManager.NameNotFoundException error) {
            // Cannot happen: it is this app asking about itself. Said rather
            // than swallowed, because if it ever does the appearance silently
            // stops being the app's.
            Log.e(TAG, "this app cannot find its own manifest, so the appearance is the default");
        }
        int mode;
        switch (appearance) {
            case "light":
                mode = androidx.appcompat.app.AppCompatDelegate.MODE_NIGHT_NO;
                break;
            case "dark":
                mode = androidx.appcompat.app.AppCompatDelegate.MODE_NIGHT_YES;
                break;
            case "system":
                mode = androidx.appcompat.app.AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM;
                break;
            default:
                // Only reachable if the CLI and this file disagree about the
                // vocabulary, which is what `check-android-appearance.sh` is
                // for. It is said, not guessed at.
                Log.e(TAG, "appearance \"" + appearance + "\" is not one this shell knows;"
                        + " following the system");
                mode = androidx.appcompat.app.AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM;
                break;
        }
        androidx.appcompat.app.AppCompatDelegate.setDefaultNightMode(mode);
    }

    private AnRuntime runtime;
    /** The viewport the engine has been told about, so it is only told changes. */
    private float viewportWidthDp;
    private float viewportHeightDp;
    private AnHost host;
    private Choreographer.FrameCallback frameCallback;
    private DevClient devClient;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        // Before `super`: `AppCompatActivity` reads the night mode while it is
        // being created, and setting it afterwards costs the activity a
        // recreation on the very first frame.
        applyAppearance();
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
        // And the modules the framework brings, which are not plugins: nobody
        // declares them and every host has them.
        AnBuiltinModules.install(this);

        host = new AnHost(this, container);
        runtime = new AnRuntime(host, widthDp, heightDp);
        if (!runtime.isValid()) {
            Log.e(TAG, "the runtime did not start");
            return;
        }
        host.attachRuntime(runtime);
        installBackDispatch();

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

        // Before the bundle is evaluated, and that ordering is the whole point.
        //
        // If the app was cold-started by a link, the URL has to be in the core
        // before Angular boots: the bundle reads whatever is waiting as it
        // starts, and the router's first decision is then already the right one.
        // Handed over afterwards it would still arrive —as an event— but one
        // navigation late, and the home screen would appear and be pushed aside.
        String launchedWith = linkOf(getIntent());
        if (launchedWith != null) {
            nativeOpenUrl(launchedWith);
        }

        // The assets carry the packaged bundle; AnBundles decides whether an
        // installed update has earned the right to run instead, and retires one
        // that failed to confirm itself on its trial launch.
        String source = AnBundles.source(this, readAsset("main.js"));
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
                        // No `host.flush()` here. The mount side already calls
                        // it once per frame it applies, and it is the only one
                        // that can: it also mounts outside this callback —a
                        // viewport change drains the worker first— and it still
                        // closes the frame when the reply carried an error and
                        // `applied` comes back as -1.
                        int applied = runtime.frame(frameTimeNanos / 1_000_000.0);
                        if (applied < 0) {
                            Log.e(TAG, "the frame failed");
                        }
                        Choreographer.getInstance().postFrameCallback(this);
                    }
                };
        Choreographer.getInstance().postFrameCallback(frameCallback);
    }

    /**
     * A link reaching an app that is already running.
     *
     * <p>It only arrives here because the activity is `singleTask` in the
     * manifest. With the default launch mode Android would build a second
     * MainActivity for the VIEW intent instead: a second engine, a second tree,
     * and the state the person had built up gone — with nothing anywhere saying
     * why.
     *
     * <p>`setIntent` matters as well. `getIntent()` goes on returning the one
     * the activity was created with until it is replaced, so a later recreation
     * —a rotation with the runtime rebuilt— would replay the launch URL and
     * navigate away from wherever the person had got to.
     */
    @Override
    protected void onNewIntent(android.content.Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        String url = linkOf(intent);
        if (url != null) {
            nativeOpenUrl(url);
        }
    }

    @Override
    public void onRequestPermissionsResult(
            int requestCode, String[] permissions, int[] granted) {
        super.onRequestPermissionsResult(requestCode, permissions, granted);
        // Nothing used to listen to this, which is why a plugin could show a
        // permission dialog and never learn what the person chose. Every plugin
        // hears it: a permission is not addressed to one of them, and two can
        // be waiting on the same answer.
        AnPluginRegistry.onPermissionResult(permissions, granted);
    }

    @Override
    @SuppressWarnings("deprecation")
    protected void onActivityResult(int requestCode, int resultCode, android.content.Intent data) {
        // A built-in that opened a system chooser is waiting for this: without
        // it the promise behind `files.pick()` would never be answered. What
        // does not belong to one is passed on, so the runtime does not swallow
        // results the app itself asked for.
        if (!AnBuiltinModules.onActivityResult(requestCode, resultCode, data)
                && !AnPluginRegistry.onActivityResult(requestCode, resultCode, data)) {
            super.onActivityResult(requestCode, resultCode, data);
        }
    }

    /**
     * The API 33+ callback, held as `Object` so that a device without the
     * `android.window` back classes never has to resolve the type to load this
     * class.
     */
    private Object backCallback;
    /** Whether that callback is on the dispatcher right now. */
    private boolean backRegistered;

    /**
     * Back, both of the ways Android has of asking for it.
     *
     * API 33 introduced `OnBackInvokedCallback` and, for an app that opts in
     * with `android:enableOnBackInvokedCallback`, stopped calling
     * `onBackPressed` at all; from API 36 the opt-out is gone and every app
     * targeting it is on the new dispatcher whether it asked or not. An app
     * that only overrides the deprecated method therefore has no back on a
     * current phone: it was measured on an Android 16 device, where back on a
     * pushed screen left the app instead of popping it.
     *
     * `minSdkVersion` is 24, so this is a second path and not a replacement.
     * On 24..32 the override below is the only back there is, and it stays.
     *
     * The callback is registered only while the app has something listening,
     * which is what `AnHost.BackHandling` reports. A callback that is always
     * on the dispatcher tells the system the app will handle every back, and
     * the system then draws no preview of its own — an app at the root of its
     * stack would lose the back-to-home animation that every other app has.
     */
    private void installBackDispatch() {
        if (android.os.Build.VERSION.SDK_INT < android.os.Build.VERSION_CODES.TIRAMISU) {
            return;
        }
        backCallback = newBackCallback();
        host.setBackHandling(this::setBackRegistered);
    }

    /**
     * The callback itself: animated from API 34, plain on 33.
     *
     * `OnBackAnimationCallback` is what predictive back is made of — the
     * gesture reports where it has got to, and the app draws the screen it is
     * about to go back to. Without it the system knows the app is handling
     * back and shows nothing at all until the finger is lifted, which is
     * worse than the old button was.
     */
    private Object newBackCallback() {
        if (android.os.Build.VERSION.SDK_INT
                >= android.os.Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            return new android.window.OnBackAnimationCallback() {
                @Override
                public void onBackStarted(android.window.BackEvent event) {
                    host.backGestureStarted();
                }

                @Override
                public void onBackProgressed(android.window.BackEvent event) {
                    host.backGestureProgress(event.getProgress());
                }

                @Override
                public void onBackCancelled() {
                    host.backGestureCancelled();
                }

                @Override
                public void onBackInvoked() {
                    host.backGestureInvoked();
                }
            };
        }
        return (android.window.OnBackInvokedCallback) () -> host.backGestureInvoked();
    }

    /** Puts the callback on the dispatcher, or takes it off. */
    private void setBackRegistered(boolean handled) {
        if (android.os.Build.VERSION.SDK_INT < android.os.Build.VERSION_CODES.TIRAMISU
                || backCallback == null
                || handled == backRegistered) {
            return;
        }
        backRegistered = handled;
        android.window.OnBackInvokedCallback callback =
                (android.window.OnBackInvokedCallback) backCallback;
        if (handled) {
            getOnBackInvokedDispatcher()
                    .registerOnBackInvokedCallback(
                            android.window.OnBackInvokedDispatcher.PRIORITY_DEFAULT, callback);
        } else {
            getOnBackInvokedDispatcher().unregisterOnBackInvokedCallback(callback);
        }
    }

    /**
     * Back on API 24 to 32, and on anything that has not honoured the opt-in.
     *
     * Deprecated since 33 and kept deliberately: it is the only back a third
     * of the supported range has. The two paths cannot both fire — the system
     * picks one per press — so there is no double dispatch to guard against.
     */
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
