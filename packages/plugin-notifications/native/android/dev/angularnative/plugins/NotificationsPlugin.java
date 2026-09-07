package dev.angularnative.plugins;

import android.Manifest;
import android.app.Activity;
import android.app.AlarmManager;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.pm.PackageManager;
import android.os.Build;

import dev.angularnative.AnEvents;
import dev.angularnative.AnPlugin;
import dev.angularnative.AnPluginCall;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * Local notifications on Android.
 *
 * <p>An immediate notification goes straight to {@link NotificationManager}. A scheduled one goes
 * through {@link AlarmManager} and a broadcast, because a process that has been killed cannot post
 * anything: the alarm belongs to the system and survives the app.
 *
 * <p>The channel is created once. Since Android 8 a notification with no channel is dropped
 * silently, which is the worst possible failure — nothing appears and nothing says why.
 */
public final class NotificationsPlugin implements AnPlugin {

    private static final String CHANNEL = "dev.angularnative.notifications";
    private static final int PERMISSION_REQUEST = 8120;
    static final String ACTION_FIRE = "dev.angularnative.NOTIFICATION_FIRE";
    static final String ACTION_TAP = "dev.angularnative.NOTIFICATION_TAP";

    private Activity host;
    private AnPluginCall asking;

    /** Ids scheduled and not yet fired, so `pending()` can answer without the system. */
    private final Map<String, Long> scheduled = new LinkedHashMap<>();

    /** Taps that arrived before JS was listening. See the Apple half for why. */
    private final java.util.List<JSONObject> held = new java.util.ArrayList<>();

    private boolean drained;
    private BroadcastReceiver taps;

    @Override
    public void attach(Activity host) {
        this.host = host;
        createChannel();
        listenForTaps();
    }

    @Override
    public void call(String method, JSONObject args, AnPluginCall respond) {
        if (host == null) {
            respond.reject("the notifications plugin has no Android context");
            return;
        }
        switch (method) {
            case "permission":
                respond.resolve(permission());
                return;

            case "request":
                // Before API 33 the permission is granted at install time and
                // there is no dialog to show.
                if (Build.VERSION.SDK_INT < 33 || !"prompt".equals(permission())) {
                    respond.resolve(permission());
                    return;
                }
                if (asking != null) {
                    asking.reject("notifications.request was already waiting for an answer");
                }
                asking = respond;
                host.requestPermissions(
                        new String[] {Manifest.permission.POST_NOTIFICATIONS}, PERMISSION_REQUEST);
                return;

            case "schedule":
                schedule(args, respond);
                return;

            case "cancel": {
                String id = args.optString("id", "");
                if (id.isEmpty()) {
                    respond.reject("notifications.cancel needs an id in 'id'");
                    return;
                }
                scheduled.remove(id);
                alarms().cancel(firePending(id, null, null, null));
                manager().cancel(id.hashCode());
                respond.resolve();
                return;
            }

            case "pending": {
                JSONArray ids = new JSONArray();
                for (String id : scheduled.keySet()) {
                    ids.put(id);
                }
                respond.resolve(ids);
                return;
            }

            case "clearDelivered":
                manager().cancelAll();
                respond.resolve();
                return;

            case "remoteToken":
                // FCM is a dependency, a google-services.json and a Firebase
                // project. Saying so is more use than an empty string somebody
                // would send to a server that could never reach this device.
                respond.reject(
                        "notifications.remoteToken needs Firebase Cloud Messaging in the app, "
                                + "which this shell does not bundle. Local notifications work; "
                                + "remote ones need a project and a server.");
                return;

            case "drain": {
                drained = true;
                for (JSONObject payload : held) {
                    AnEvents.emit("notifications", "notification", payload);
                }
                held.clear();
                respond.resolve();
                return;
            }

            default:
                respond.reject("the notifications plugin has no method " + method);
        }
    }

    private void schedule(JSONObject args, AnPluginCall respond) {
        String id = args.optString("id", "");
        if (id.isEmpty()) {
            respond.reject("notifications.schedule needs an id in 'id'");
            return;
        }
        if (!args.has("title")) {
            respond.reject("notifications.schedule needs a title in 'title'");
            return;
        }
        String title = args.optString("title");
        String body = args.optString("body", "");
        String data = args.optJSONObject("data") == null ? "{}" : args.optJSONObject("data").toString();
        double at = args.optDouble("at", 0);

        if (at <= 0 || at <= System.currentTimeMillis()) {
            // A time that has gone means now, which is what the caller meant.
            post(id, title, body, data);
            respond.resolve();
            return;
        }
        // Scheduling with an id that is pending replaces it: the PendingIntent
        // is keyed by the id, so the second one overwrites the first.
        scheduled.put(id, (long) at);
        alarms().setExact(
                AlarmManager.RTC_WAKEUP, (long) at, firePending(id, title, body, data));
        respond.resolve();
    }

    /** Puts one on screen now. Also called by the receiver when an alarm fires. */
    void post(String id, String title, String body, String data) {
        scheduled.remove(id);
        Intent tapped = new Intent(ACTION_TAP).setPackage(host.getPackageName());
        tapped.putExtra("id", id);
        tapped.putExtra("title", title);
        tapped.putExtra("body", body);
        tapped.putExtra("data", data);
        PendingIntent open =
                PendingIntent.getBroadcast(
                        host,
                        ("tap:" + id).hashCode(),
                        tapped,
                        PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);

        Notification notification =
                new Notification.Builder(host, CHANNEL)
                        .setContentTitle(title)
                        .setContentText(body)
                        .setSmallIcon(host.getApplicationInfo().icon)
                        .setAutoCancel(true)
                        .setContentIntent(open)
                        .build();
        manager().notify(id.hashCode(), notification);
    }

    private PendingIntent firePending(String id, String title, String body, String data) {
        Intent intent = new Intent(ACTION_FIRE).setPackage(host.getPackageName());
        intent.putExtra("id", id);
        intent.putExtra("title", title);
        intent.putExtra("body", body);
        intent.putExtra("data", data);
        return PendingIntent.getBroadcast(
                host,
                id.hashCode(),
                intent,
                PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
    }

    private void listenForTaps() {
        taps =
                new BroadcastReceiver() {
                    @Override
                    public void onReceive(Context context, Intent intent) {
                        JSONObject payload = describe(intent, ACTION_TAP.equals(intent.getAction()));
                        if (ACTION_FIRE.equals(intent.getAction())) {
                            post(
                                    intent.getStringExtra("id"),
                                    intent.getStringExtra("title"),
                                    intent.getStringExtra("body"),
                                    intent.getStringExtra("data"));
                            return;
                        }
                        if (drained) {
                            AnEvents.emit("notifications", "notification", payload);
                        } else {
                            held.add(payload);
                        }
                    }
                };
        IntentFilter filter = new IntentFilter();
        filter.addAction(ACTION_TAP);
        filter.addAction(ACTION_FIRE);
        if (Build.VERSION.SDK_INT >= 33) {
            host.registerReceiver(taps, filter, Context.RECEIVER_NOT_EXPORTED);
        } else {
            host.registerReceiver(taps, filter);
        }
    }

    private JSONObject describe(Intent intent, boolean tapped) {
        JSONObject out = new JSONObject();
        try {
            out.put("id", String.valueOf(intent.getStringExtra("id")));
            out.put("title", String.valueOf(intent.getStringExtra("title")));
            out.put("body", String.valueOf(intent.getStringExtra("body")));
            String data = intent.getStringExtra("data");
            out.put("data", data == null ? new JSONObject() : new JSONObject(data));
            out.put("tapped", tapped);
        } catch (JSONException malformed) {
            // The data came from JS as JSON; if it is not, an empty object is
            // better than losing the whole event.
            try {
                out.put("data", new JSONObject());
            } catch (JSONException impossible) {
                // A string key and an object value.
            }
        }
        return out;
    }

    private void createChannel() {
        // Since Android 8 a notification with no channel is dropped in silence,
        // which is the worst kind of failure: nothing appears and nothing says
        // why.
        NotificationChannel channel =
                new NotificationChannel(
                        CHANNEL, "Notifications", NotificationManager.IMPORTANCE_DEFAULT);
        manager().createNotificationChannel(channel);
    }

    private String permission() {
        if (Build.VERSION.SDK_INT < 33) {
            return "granted";
        }
        if (host.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)
                == PackageManager.PERMISSION_GRANTED) {
            return "granted";
        }
        return host.shouldShowRequestPermissionRationale(Manifest.permission.POST_NOTIFICATIONS)
                ? "denied"
                : "prompt";
    }

    @Override
    public void onPermissionResult(String[] permissions, int[] granted) {
        if (asking == null) {
            return;
        }
        for (String permission : permissions) {
            if (Manifest.permission.POST_NOTIFICATIONS.equals(permission)) {
                AnPluginCall waiting = asking;
                asking = null;
                waiting.resolve(permission());
                return;
            }
        }
    }

    private NotificationManager manager() {
        return (NotificationManager) host.getSystemService(Context.NOTIFICATION_SERVICE);
    }

    private AlarmManager alarms() {
        return (AlarmManager) host.getSystemService(Context.ALARM_SERVICE);
    }
}
