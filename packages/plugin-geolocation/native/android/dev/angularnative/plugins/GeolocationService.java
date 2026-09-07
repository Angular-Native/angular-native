package dev.angularnative.plugins;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.location.Location;
import android.location.LocationListener;
import android.location.LocationManager;
import android.os.Build;
import android.os.IBinder;
import android.os.Looper;

import dev.angularnative.AnEvents;

import org.json.JSONException;
import org.json.JSONObject;

/**
 * Keeping the location stream alive once the app is off screen.
 *
 * <p>Android stops delivering to a backgrounded process, and the only supported way to carry on is
 * a foreground service — which means <b>a notification the person can see for as long as it
 * runs</b>. That is not an obstacle to route around: the platform requires it exactly so that an
 * app cannot follow somebody quietly, and an app that wants background location owes them that
 * notice.
 *
 * <p>It emits through {@link AnEvents} under the same {@code geolocation.position} name the
 * foreground watch uses, so nothing in JS has to know which of the two produced a fix.
 */
public final class GeolocationService extends Service {

    private static final String CHANNEL = "dev.angularnative.geolocation";
    private static final int NOTIFICATION = 4243;

    static final String EXTRA_MIN_METRES = "minMetres";

    private LocationManager manager;
    private LocationListener listener;

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        startForeground(NOTIFICATION, notification());
        float metres = intent == null ? 0f : intent.getFloatExtra(EXTRA_MIN_METRES, 0f);

        manager = (LocationManager) getSystemService(Context.LOCATION_SERVICE);
        if (manager == null) {
            stopSelf();
            return START_NOT_STICKY;
        }
        listener =
                new LocationListener() {
                    @Override
                    public void onLocationChanged(Location fix) {
                        AnEvents.emit("geolocation", "position", describe(fix));
                    }

                    @Override
                    public void onProviderDisabled(String provider) {}

                    @Override
                    public void onProviderEnabled(String provider) {}

                    @Override
                    public void onStatusChanged(String p, int s, android.os.Bundle e) {}
                };
        try {
            String provider =
                    manager.isProviderEnabled(LocationManager.GPS_PROVIDER)
                            ? LocationManager.GPS_PROVIDER
                            : LocationManager.NETWORK_PROVIDER;
            manager.requestLocationUpdates(provider, 0L, metres, listener, Looper.getMainLooper());
        } catch (SecurityException refused) {
            AnEvents.emit("geolocation", "error", error("the background fix was refused: " + refused));
            stopSelf();
            return START_NOT_STICKY;
        }
        // START_STICKY would have Android restart this with a null intent after
        // the process is killed, losing the distance filter and starting a watch
        // nobody asked for. If the app dies, the watch dies with it.
        return START_NOT_STICKY;
    }

    @Override
    public void onDestroy() {
        if (manager != null && listener != null) {
            try {
                manager.removeUpdates(listener);
            } catch (SecurityException ignored) {
                // Losing the permission while running is not worth a crash.
            }
        }
        listener = null;
        super.onDestroy();
    }

    private Notification notification() {
        NotificationManager notifications =
                (NotificationManager) getSystemService(Context.NOTIFICATION_SERVICE);
        // Low importance: it has to be visible, it does not have to interrupt.
        NotificationChannel channel =
                new NotificationChannel(
                        CHANNEL, "Location", NotificationManager.IMPORTANCE_LOW);
        channel.setShowBadge(false);
        notifications.createNotificationChannel(channel);

        Notification.Builder builder =
                new Notification.Builder(this, CHANNEL)
                        .setContentTitle("Following your location")
                        .setContentText("This app is using your location in the background.")
                        .setSmallIcon(getApplicationInfo().icon)
                        .setOngoing(true);
        if (Build.VERSION.SDK_INT >= 31) {
            builder.setForegroundServiceBehavior(Notification.FOREGROUND_SERVICE_IMMEDIATE);
        }
        return builder.build();
    }

    private static JSONObject error(String message) {
        JSONObject out = new JSONObject();
        try {
            out.put("message", message);
        } catch (JSONException impossible) {
            // A string key and a string value.
        }
        return out;
    }

    /** The same shape the foreground watch emits, so JS cannot tell them apart. */
    static JSONObject describe(Location fix) {
        JSONObject out = new JSONObject();
        try {
            out.put("latitude", fix.getLatitude());
            out.put("longitude", fix.getLongitude());
            out.put("accuracy", fix.getAccuracy());
            out.put("timestamp", fix.getTime());
            out.put("altitude", fix.hasAltitude() ? (Object) fix.getAltitude() : JSONObject.NULL);
            out.put("speed", fix.hasSpeed() ? (Object) fix.getSpeed() : JSONObject.NULL);
            out.put("heading", fix.hasBearing() ? (Object) fix.getBearing() : JSONObject.NULL);
        } catch (JSONException impossible) {
            // Every key is a string and every value a primitive.
        }
        return out;
    }
}
