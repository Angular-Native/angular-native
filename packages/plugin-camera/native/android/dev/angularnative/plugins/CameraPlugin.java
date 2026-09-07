package dev.angularnative.plugins;

import android.Manifest;
import android.app.Activity;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.net.Uri;
import android.os.Build;
import android.provider.MediaStore;

import androidx.core.content.FileProvider;

import dev.angularnative.AnPlugin;
import dev.angularnative.AnPluginCall;
import dev.angularnative.AnPluginRegistry;

import org.json.JSONException;
import org.json.JSONObject;

import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;

/**
 * The camera and the photo picker on Android.
 *
 * <p>Both are other apps: {@code ACTION_IMAGE_CAPTURE} and the system photo picker. That is why
 * this plugin needs a request code and {@link #onActivityResult} — the answer comes back on the
 * Activity, seconds later, not inside {@code call}.
 *
 * <p><b>The camera writes into our own cache and not into the gallery.</b> A photograph the person
 * has not decided to keep should not appear in their camera roll, and writing there would need
 * storage permissions this plugin deliberately does not ask for. The file goes to
 * {@code getCacheDir()} and travels to the camera app as a {@code content://} URI through the
 * shell's existing {@code FileProvider}.
 */
public final class CameraPlugin implements AnPlugin {

    /**
     * Must match {@code android:authorities} in the manifest, after the application id — the same
     * suffix {@code AnShare} uses, and for the same provider.
     *
     * <p>It is a suffix and not the whole thing because a provider authority is unique across the
     * device rather than the app: a literal one would mean two angular-native apps could not be
     * installed at once. The manifest writes it as {@code ${applicationId}.anfiles} and {@code an}
     * substitutes it, so the only thing that can be said here is the half that does not change.
     */
    private static final String AUTHORITY_SUFFIX = ".anfiles";
    private static final int PERMISSION_REQUEST = 7311;

    private Activity host;
    private int captureCode;
    private int pickCode;

    private AnPluginCall pending;
    private AnPluginCall asking;
    private File target;
    private int maxSize;
    private int quality = 85;

    @Override
    public void attach(Activity host) {
        this.host = host;
        // Reserved once, for the life of the process: the Activity can be
        // recreated while the camera is on screen, and the result comes back to
        // whatever is standing there afterwards.
        this.captureCode = AnPluginRegistry.reserveRequestCode(this);
        this.pickCode = AnPluginRegistry.reserveRequestCode(this);
    }

    @Override
    public void call(String method, JSONObject args, AnPluginCall respond) {
        if (host == null) {
            respond.reject("the camera plugin has no Android context");
            return;
        }
        switch (method) {
            case "permission":
                respond.resolve(permission());
                return;

            case "request":
                if (!"prompt".equals(permission())) {
                    respond.resolve(permission());
                    return;
                }
                if (asking != null) {
                    asking.reject("camera.request was already waiting for an answer");
                }
                asking = respond;
                host.requestPermissions(
                        new String[] {Manifest.permission.CAMERA}, PERMISSION_REQUEST);
                return;

            case "takePhoto":
                takePhoto(args, respond);
                return;

            case "pickPhoto":
                pickPhoto(args, respond);
                return;

            default:
                respond.reject("the camera plugin has no method " + method);
        }
    }

    private String permission() {
        if (host.checkSelfPermission(Manifest.permission.CAMERA)
                == PackageManager.PERMISSION_GRANTED) {
            return "granted";
        }
        return host.shouldShowRequestPermissionRationale(Manifest.permission.CAMERA)
                ? "denied"
                : "prompt";
    }

    private void takePhoto(JSONObject args, AnPluginCall respond) {
        if (!"granted".equals(permission())) {
            respond.reject(
                    "camera.takePhoto has no permission: the camera is off for this app in"
                            + " Settings");
            return;
        }
        if (!take(args, respond)) {
            return;
        }
        try {
            File directory = new File(host.getCacheDir(), "an-camera");
            if (!directory.exists() && !directory.mkdirs()) {
                fail("camera: the cache directory could not be created");
                return;
            }
            target = new File(directory, "an-camera-" + System.currentTimeMillis() + ".jpg");
            Uri destination =
                    FileProvider.getUriForFile(
                            host, host.getPackageName() + AUTHORITY_SUFFIX, target);
            Intent intent = new Intent(MediaStore.ACTION_IMAGE_CAPTURE);
            intent.putExtra(MediaStore.EXTRA_OUTPUT, destination);
            // Without this the camera app cannot write to the URI it was given,
            // and comes back with a result nobody can read.
            intent.addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
            if (intent.resolveActivity(host.getPackageManager()) == null) {
                fail("camera.takePhoto found no camera app on this device");
                return;
            }
            host.startActivityForResult(intent, captureCode);
        } catch (RuntimeException error) {
            fail("camera.takePhoto could not start: " + error);
        }
    }

    private void pickPhoto(JSONObject args, AnPluginCall respond) {
        if (!take(args, respond)) {
            return;
        }
        Intent intent;
        if (Build.VERSION.SDK_INT >= 33) {
            // The photo picker runs outside the app and hands back only what was
            // chosen, so there is no storage permission to ask for.
            intent = new Intent(MediaStore.ACTION_PICK_IMAGES);
            intent.setType("image/*");
        } else {
            intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            intent.addCategory(Intent.CATEGORY_OPENABLE);
            intent.setType("image/*");
        }
        try {
            host.startActivityForResult(intent, pickCode);
        } catch (RuntimeException error) {
            fail("camera.pickPhoto could not open a picker: " + error);
        }
    }

    private boolean take(JSONObject args, AnPluginCall respond) {
        if (pending != null) {
            respond.reject("camera: another picker is already open");
            return false;
        }
        pending = respond;
        maxSize = (int) args.optDouble("maxSize", 0);
        quality = (int) Math.round(args.optDouble("quality", 0.85) * 100);
        target = null;
        return true;
    }

    @Override
    public void onPermissionResult(String[] permissions, int[] granted) {
        if (asking == null) {
            return;
        }
        for (String permission : permissions) {
            if (Manifest.permission.CAMERA.equals(permission)) {
                AnPluginCall waiting = asking;
                asking = null;
                waiting.resolve(permission());
                return;
            }
        }
    }

    @Override
    public void onActivityResult(int resultCode, Intent data) {
        AnPluginCall waiting = pending;
        pending = null;
        if (waiting == null) {
            return;
        }
        if (resultCode != Activity.RESULT_OK) {
            // Cancelling says so, so an app that only catches can still tell it
            // from a camera that is broken.
            waiting.reject("camera: the picker was cancelled");
            return;
        }
        try {
            if (target != null && target.exists()) {
                answer(waiting, BitmapFactory.decodeFile(target.getAbsolutePath()), target);
                return;
            }
            Uri chosen = data == null ? null : data.getData();
            if (chosen == null) {
                waiting.reject("camera: the picker came back with nothing");
                return;
            }
            try (InputStream stream = host.getContentResolver().openInputStream(chosen)) {
                answer(waiting, BitmapFactory.decodeStream(stream), null);
            }
        } catch (Exception error) {
            waiting.reject("camera: the photograph could not be read: " + error);
        }
    }

    /**
     * Scales, writes and answers with the path.
     *
     * <p>Nothing crosses the bridge as bytes: a twelve-megapixel photograph as base64 is sixteen
     * megabytes of string through a JSON boundary, encoded once and parsed once. A path costs a
     * few dozen.
     */
    private void answer(AnPluginCall waiting, Bitmap image, File wroteTo) throws Exception {
        if (image == null) {
            waiting.reject("camera: the photograph could not be decoded");
            return;
        }
        Bitmap scaled = scale(image, maxSize);
        File out =
                wroteTo != null
                        ? wroteTo
                        : new File(
                                cacheDirectory(),
                                "an-camera-" + System.currentTimeMillis() + ".jpg");
        try (FileOutputStream stream = new FileOutputStream(out)) {
            scaled.compress(Bitmap.CompressFormat.JPEG, Math.max(1, Math.min(100, quality)), stream);
        }
        JSONObject photo = new JSONObject();
        try {
            photo.put("path", out.getAbsolutePath());
            photo.put("width", scaled.getWidth());
            photo.put("height", scaled.getHeight());
        } catch (JSONException impossible) {
            // Two ints and a string.
        }
        waiting.resolve(photo);
    }

    private File cacheDirectory() {
        File directory = new File(host.getCacheDir(), "an-camera");
        //noinspection ResultOfMethodCallIgnored
        directory.mkdirs();
        return directory;
    }

    private static Bitmap scale(Bitmap image, int longestSide) {
        int longest = Math.max(image.getWidth(), image.getHeight());
        if (longestSide <= 0 || longest <= longestSide) {
            return image;
        }
        float ratio = (float) longestSide / longest;
        return Bitmap.createScaledBitmap(
                image,
                Math.round(image.getWidth() * ratio),
                Math.round(image.getHeight() * ratio),
                true);
    }

    private void fail(String why) {
        AnPluginCall waiting = pending;
        pending = null;
        if (waiting != null) {
            waiting.reject(why);
        }
    }
}
