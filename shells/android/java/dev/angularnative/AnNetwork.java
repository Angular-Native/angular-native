package dev.angularnative;

import android.app.Activity;
import android.content.Context;
import android.content.pm.PackageManager;
import android.net.ConnectivityManager;
import android.net.Network;
import android.net.NetworkCapabilities;

import org.json.JSONObject;

/**
 * The {@code network} module on Android and Wear OS.
 *
 * <p>It is {@link ConnectivityManager.NetworkCallback}, which is the system's own view of the way
 * out: not a reachability check against some address and not a request that failed, but what the
 * platform itself believes about the default network, pushed when the Wi-Fi drops and when the phone
 * falls back to mobile data.
 *
 * <p>The callback is registered once, at attach, and left running. That is what lets {@code status}
 * answer immediately and from the truth rather than going off to ask a question that takes time.
 *
 * <p>It is also the one built-in that needs a manifest permission. {@code ACCESS_NETWORK_STATE} is
 * an install-time permission, so there is no dialog and nothing to ask the person: it is either in
 * the manifest or every call throws {@code SecurityException}. This module checks first and rejects
 * with the line to paste, which is the difference between an app that tells its developer what is
 * missing and an app that dies at the first call.
 */
public final class AnNetwork implements AnBuiltinModule {

    private Activity host;
    private ConnectivityManager connectivity;

    /** What the callback last said, or null before its first word. */
    private volatile NetworkCapabilities capabilities;

    private volatile boolean online;

    /** Set when the permission is missing, so the message is built once. */
    private boolean permitted;

    @Override
    public void attach(Activity host) {
        this.host = host;
        this.permitted =
                host.checkSelfPermission(android.Manifest.permission.ACCESS_NETWORK_STATE)
                        == PackageManager.PERMISSION_GRANTED;
        if (!permitted) {
            return;
        }
        connectivity = (ConnectivityManager) host.getSystemService(Context.CONNECTIVITY_SERVICE);
        if (connectivity == null) {
            return;
        }
        try {
            connectivity.registerDefaultNetworkCallback(
                    new ConnectivityManager.NetworkCallback() {
                        @Override
                        public void onAvailable(Network network) {
                            online = true;
                        }

                        @Override
                        public void onLost(Network network) {
                            online = false;
                            capabilities = null;
                        }

                        @Override
                        public void onCapabilitiesChanged(
                                Network network, NetworkCapabilities changed) {
                            capabilities = changed;
                            online = changed.hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED)
                                    || changed.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET);
                        }
                    });
        } catch (RuntimeException error) {
            // Registering can fail on a device with no connectivity service at
            // all. The module still answers; it answers "offline", which is
            // what a device with no way out is.
            connectivity = null;
        }
    }

    @Override
    public void call(String method, JSONObject args, AnBuiltinCall respond) {
        if (!"status".equals(method)) {
            respond.reject("the network module has no method " + method);
            return;
        }
        if (!permitted) {
            respond.reject(
                    "network.status cannot read the network: this app has no"
                            + " ACCESS_NETWORK_STATE. It is an install-time permission, so there is"
                            + " no dialog to show and nothing the person can grant at run time —"
                            + " add this line to your AndroidManifest.xml, above <application>:\n"
                            + "  <uses-permission"
                            + " android:name=\"android.permission.ACCESS_NETWORK_STATE\" />");
            return;
        }
        JSONObject status = new JSONObject();
        NetworkCapabilities current = capabilities;
        try {
            status.put("online", online && current != null);
            status.put("connection", connection(current));
            // A metered connection: mobile data, or a phone's hotspot. What an
            // app consults before downloading something large.
            status.put(
                    "expensive",
                    current != null
                            && !current.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED));
            // Data Saver, which is Android's half of what iOS calls Low Data
            // Mode: the person asked for less traffic and the system says so.
            status.put("constrained", dataSaverOn());
        } catch (org.json.JSONException error) {
            respond.reject("network.status could not be built: " + error);
            return;
        }
        respond.resolve(status);
    }

    /**
     * Which transport it is going out through.
     *
     * <p>{@code "none"} is not a transport called none: it is what there is when the callback has
     * said nothing or has said the network is gone. Calling that {@code "other"} would be reporting
     * a connection where there is none.
     */
    private static String connection(NetworkCapabilities capabilities) {
        if (capabilities == null) {
            return "none";
        }
        if (capabilities.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)) {
            return "wifi";
        }
        if (capabilities.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR)) {
            return "cellular";
        }
        if (capabilities.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET)) {
            return "ethernet";
        }
        return "other";
    }

    private boolean dataSaverOn() {
        if (connectivity == null) {
            return false;
        }
        try {
            return connectivity.getRestrictBackgroundStatus()
                    == ConnectivityManager.RESTRICT_BACKGROUND_STATUS_ENABLED;
        } catch (RuntimeException error) {
            return false;
        }
    }
}
