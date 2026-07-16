package org.freetakteam.rem.plugin.watchstatus;

import android.content.Context;
import android.content.SharedPreferences;
import org.json.JSONException;
import org.json.JSONObject;

final class WatchStatusSettings {
    static final int DEFAULT_PORT = 29_863;
    private static final String PREFS = "watch-status-plugin";
    private final SharedPreferences preferences;

    WatchStatusSettings(Context context) {
        preferences = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
    }

    boolean enabled() { return preferences.getBoolean("enabled", true); }
    int port() { return normalizePort(preferences.getInt("port", DEFAULT_PORT)); }

    void update(JSONObject request) throws JSONException {
        final boolean enabled = request.has("enabled") ? request.getBoolean("enabled") : enabled();
        final int requestedPort = request.has("port") ? request.getInt("port") : port();
        if (!isValidPort(requestedPort)) throw new JSONException("Port must be between 1024 and 65535");
        preferences.edit().putBoolean("enabled", enabled).putInt("port", requestedPort).apply();
    }

    JSONObject json(boolean running, String bindError, long snapshotAgeMs) throws JSONException {
        return new JSONObject()
            .put("type", "state")
            .put("enabled", enabled())
            .put("port", port())
            .put("url", "http://localhost:" + port() + "/info.json")
            .put("running", running)
            .put("bindError", bindError == null ? "" : bindError)
            .put("snapshotAgeMs", snapshotAgeMs);
    }

    static boolean isValidPort(int value) { return value >= 1_024 && value <= 65_535; }
    static int normalizePort(int value) { return isValidPort(value) ? value : DEFAULT_PORT; }
}
