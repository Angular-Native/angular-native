package dev.angularnative.plugins;

import android.Manifest;
import android.app.Activity;
import android.content.Context;
import android.content.pm.PackageManager;
import android.location.Location;
import android.location.LocationListener;
import android.location.LocationManager;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;

import dev.angularnative.AnEvents;
import dev.angularnative.AnPlugin;
import dev.angularnative.AnPluginCall;

import org.json.JSONException;
import org.json.JSONObject;

import java.util.List;

/**
 * The device's location on Android, through the platform's own {@link LocationManager}.
 *
 * <p><b>Not Google's fused provider.</b> {@code FusedLocationProviderClient} gives better fixes and
 * lives in Play Services, which is not on every Android device — a plugin that depended on it would
 * make an app that cannot be installed on the ones without it. The platform API is on all of them.
 *
 * <p>Like the Apple half, this one cannot answer on the spot: a fix arrives on a listener some
 * seconds later, so the {@link AnPluginCall} is held and answered from there. Everything that can
 * end the wait — a fix, a provider that goes away, the timeout — goes through one place, so the
 * promise is settled exactly once.
 */
public final class GeolocationPlugin implements AnPlugin {

    /** Android has no permission dialog an app can trigger from here without an Activity result
     *  plumbing the shell does not expose, so a refusal is reported rather than re-asked. */
    private static final String FINE = Manifest.permission.ACCESS_FINE_LOCATION;
    private static final String COARSE = Manifest.permission.ACCESS_COARSE_LOCATION;

    private Activity host;
    private final Handler main = new Handler(Looper.getMainLooper());

    private AnPluginCall pending;
    private Runnable timeout;
    private LocationListener listener;

    /** The stream's own listener, kept apart from the one-shot's: a `current()`
     *  that finishes must not tear down a watch that is still running. */
    private LocationListener watcher;

    @Override
    public void attach(Activity host) {
        this.host = host;
    }

    @Override
    public void call(String method, JSONObject args, AnPluginCall respond) {
        if (host == null) {
            respond.reject("the geolocation plugin has no Android context");
            return;
        }
        switch (method) {
            case "permission":
                respond.resolve(permission());
                return;

            case "request":
                // On Android the grant is a system dialog owned by the Activity,
                // and its result comes back through onRequestPermissionsResult.
                // Until the shell forwards that, asking would show the dialog and
                // then never hear the answer — so the standing state is returned
                // and the app is told where the switch is.
                if ("prompt".equals(permission())) {
                    host.requestPermissions(new String[] {FINE, COARSE}, 4242);
                }
                respond.resolve(permission());
                return;

            case "current":
                current(args, respond);
                return;

            case "watch":
                watch(args, respond);
                return;

            case "unwatch":
                unwatch();
                respond.resolve();
                return;

            default:
                respond.reject("the geolocation plugin has no method " + method);
        }
    }

    private String permission() {
        if (host.checkSelfPermission(FINE) == PackageManager.PERMISSION_GRANTED
                || host.checkSelfPermission(COARSE) == PackageManager.PERMISSION_GRANTED) {
            return "granted";
        }
        // `shouldShowRequestPermissionRationale` is false both before the first
        // ask and after "don't ask again", so it is what separates "not decided"
        // from "decided no, and no dialog will come back".
        if (host.shouldShowRequestPermissionRationale(FINE)) {
            return "denied";
        }
        return "prompt";
    }

    private void current(JSONObject args, AnPluginCall respond) {
        if (!"granted".equals(permission())) {
            respond.reject(
                    "geolocation.current has no permission: location for this app is off in"
                            + " Settings");
            return;
        }
        LocationManager manager =
                (LocationManager) host.getSystemService(Context.LOCATION_SERVICE);
        if (manager == null) {
            respond.reject("geolocation.current found no LocationManager on this device");
            return;
        }
        if (!manager.isProviderEnabled(LocationManager.GPS_PROVIDER)
                && !manager.isProviderEnabled(LocationManager.NETWORK_PROVIDER)) {
            respond.reject(
                    "geolocation.current cannot run: location is switched off on this device");
            return;
        }
        if (pending != null) {
            pending.reject("geolocation.current was superseded by another call");
            stop();
        }

        pending = respond;
        long milliseconds = (long) args.optDouble("timeoutMs", 10_000);

        listener =
                new LocationListener() {
                    @Override
                    public void onLocationChanged(Location fix) {
                        AnPluginCall waiting = settle();
                        if (waiting != null) {
                            waiting.resolve(describe(fix));
                        }
                    }

                    // The three-argument overloads are abstract before API 30 and
                    // default after it. They are implemented so the class compiles
                    // against either.
                    @Override
                    public void onProviderDisabled(String provider) {}

                    @Override
                    public void onProviderEnabled(String provider) {}

                    @Override
                    public void onStatusChanged(String provider, int status, android.os.Bundle e) {}
                };

        timeout =
                () -> {
                    AnPluginCall waiting = settle();
                    if (waiting != null) {
                        waiting.reject(
                                "geolocation.current had no fix after " + milliseconds + " ms");
                    }
                };
        main.postDelayed(timeout, milliseconds);

        try {
            // A single update rather than a stream: this is a one-shot call, and
            // `requestLocationUpdates` would keep the radio on until somebody
            // remembered to stop it.
            String provider =
                    manager.isProviderEnabled(LocationManager.GPS_PROVIDER)
                            ? LocationManager.GPS_PROVIDER
                            : LocationManager.NETWORK_PROVIDER;
            if (Build.VERSION.SDK_INT >= 30) {
                manager.getCurrentLocation(
                        provider,
                        null,
                        host.getMainExecutor(),
                        fix -> {
                            AnPluginCall waiting = settle();
                            if (waiting == null) {
                                return;
                            }
                            if (fix == null) {
                                waiting.reject("geolocation.current got no fix from " + provider);
                            } else {
                                waiting.resolve(describe(fix));
                            }
                        });
            } else {
                manager.requestSingleUpdate(provider, listener, Looper.getMainLooper());
            }
        } catch (SecurityException refused) {
            AnPluginCall waiting = settle();
            if (waiting != null) {
                waiting.reject("geolocation.current was refused by the system: " + refused);
            }
        }
    }

    private void watch(JSONObject args, AnPluginCall respond) {
        if (!"granted".equals(permission())) {
            respond.reject(
                    "geolocation.watch has no permission: location for this app is off in"
                            + " Settings");
            return;
        }
        LocationManager manager =
                (LocationManager) host.getSystemService(Context.LOCATION_SERVICE);
        if (manager == null) {
            respond.reject("geolocation.watch found no LocationManager on this device");
            return;
        }
        unwatch();

        // A distance filter rather than a timer: the platform already knows the
        // device has not moved, and polling would either miss a movement or keep
        // the radio awake for nothing.
        float metres = (float) args.optDouble("minMetres", 0);
        watcher =
                new LocationListener() {
                    @Override
                    public void onLocationChanged(Location fix) {
                        // A watch emits; it does not answer. The event belongs to
                        // no call, which is the whole reason the channel exists.
                        AnEvents.emit("geolocation", "position", describe(fix));
                    }

                    @Override
                    public void onProviderDisabled(String provider) {
                        // A stream cannot reject: nobody holds a promise for it.
                        JSONObject why = new JSONObject();
                        try {
                            why.put("message", provider + " was switched off");
                        } catch (JSONException impossible) {
                            // The key is a string and the value is a string.
                        }
                        AnEvents.emit("geolocation", "error", why);
                    }

                    @Override
                    public void onProviderEnabled(String provider) {}

                    @Override
                    public void onStatusChanged(String provider, int status, android.os.Bundle e) {}
                };

        try {
            String provider =
                    manager.isProviderEnabled(LocationManager.GPS_PROVIDER)
                            ? LocationManager.GPS_PROVIDER
                            : LocationManager.NETWORK_PROVIDER;
            manager.requestLocationUpdates(provider, 0L, metres, watcher, Looper.getMainLooper());
            respond.resolve();
        } catch (SecurityException refused) {
            watcher = null;
            respond.reject("geolocation.watch was refused by the system: " + refused);
        }
    }

    private void unwatch() {
        if (watcher == null || host == null) {
            return;
        }
        LocationManager manager =
                (LocationManager) host.getSystemService(Context.LOCATION_SERVICE);
        if (manager != null) {
            try {
                manager.removeUpdates(watcher);
            } catch (SecurityException ignored) {
                // Losing the permission while watching is not worth a crash.
            }
        }
        watcher = null;
    }

    /** Takes the waiting call and tears everything down, or null if something got there first. */
    private AnPluginCall settle() {
        AnPluginCall waiting = pending;
        pending = null;
        stop();
        return waiting;
    }

    private void stop() {
        if (timeout != null) {
            main.removeCallbacks(timeout);
            timeout = null;
        }
        if (listener != null && host != null) {
            LocationManager manager =
                    (LocationManager) host.getSystemService(Context.LOCATION_SERVICE);
            if (manager != null) {
                try {
                    manager.removeUpdates(listener);
                } catch (SecurityException ignored) {
                    // Losing the permission between the request and the answer is
                    // not something to crash over.
                }
            }
            listener = null;
        }
    }

    private static JSONObject describe(Location fix) {
        JSONObject out = new JSONObject();
        try {
            out.put("latitude", fix.getLatitude());
            out.put("longitude", fix.getLongitude());
            out.put("accuracy", fix.getAccuracy());
            out.put("timestamp", fix.getTime());
            // `hasAltitude` and friends are how Android says "this field is not
            // filled in". Without them a device with no barometer reports zero
            // metres above sea level, which is a number and not an answer.
            out.put("altitude", fix.hasAltitude() ? (Object) fix.getAltitude() : JSONObject.NULL);
            out.put("speed", fix.hasSpeed() ? (Object) fix.getSpeed() : JSONObject.NULL);
            out.put("heading", fix.hasBearing() ? (Object) fix.getBearing() : JSONObject.NULL);
        } catch (JSONException impossible) {
            // Every key is a string and every value a primitive.
        }
        return out;
    }
}
