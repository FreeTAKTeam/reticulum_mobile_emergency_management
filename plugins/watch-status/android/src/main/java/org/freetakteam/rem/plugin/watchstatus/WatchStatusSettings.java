package org.freetakteam.rem.plugin.watchstatus;

import android.content.Context;
import android.content.SharedPreferences;
import org.json.JSONException;
import org.json.JSONObject;

final class WatchStatusSettings implements RadarAccess.Credentials {
    static final int DEFAULT_PORT = 29_863;
    private static final String PREFS = "watch-status-plugin";
    private final SharedPreferences preferences;

    WatchStatusSettings(Context context) {
        preferences = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
    }

    boolean enabled() { return preferences.getBoolean("enabled", true); }
    int port() { return normalizePort(preferences.getInt("port", DEFAULT_PORT)); }
    boolean radarEnabled() { return preferences.getBoolean("radarEnabled", false); }

    @Override public String read() { return preferences.getString("radarPairingToken", ""); }

    @Override public void write(String token) {
        final SharedPreferences.Editor editor = preferences.edit();
        if (token.isEmpty()) editor.remove("radarPairingToken");
        else editor.putString("radarPairingToken", token);
        if (!editor.commit()) throw new IllegalStateException("Unable to save radar pairing");
    }

    void update(JSONObject request) throws JSONException {
        final boolean enabled = request.has("enabled") ? request.getBoolean("enabled") : enabled();
        final int requestedPort = request.has("port") ? request.getInt("port") : port();
        if (!isValidPort(requestedPort)) throw new JSONException("Port must be between 1024 and 65535");
        final boolean radar = request.has("radarEnabled") ? request.getBoolean("radarEnabled") : radarEnabled();
        final SharedPreferences.Editor editor = preferences.edit().putBoolean("enabled", enabled).putInt("port", requestedPort)
            .putBoolean("radarEnabled", radar);
        if (!radar) editor.remove("radarPairingToken");
        if (!editor.commit()) throw new IllegalStateException("Unable to save server settings");
    }

    JSONObject json(boolean running, String bindError, long snapshotAgeMs) throws JSONException {
        return new JSONObject()
            .put("type", "state")
            .put("enabled", enabled())
            .put("port", port())
            .put("radarEnabled", radarEnabled())
            .put("radarUrl", "http://127.0.0.1:" + port() + "/northbound/watch/radar")
            .put("url", "http://localhost:" + port() + "/info.json")
            .put("running", running)
            .put("bindError", bindError == null ? "" : bindError)
            .put("snapshotAgeMs", snapshotAgeMs);
    }

    static boolean isValidPort(int value) { return value >= 1_024 && value <= 65_535; }
    static int normalizePort(int value) { return isValidPort(value) ? value : DEFAULT_PORT; }
}
