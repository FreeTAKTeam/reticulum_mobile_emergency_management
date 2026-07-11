package org.freetakteam.rem.plugin.bleheartrate;

import android.content.Context;
import android.content.SharedPreferences;
import java.util.Locale;
import org.json.JSONObject;

final class PluginPreferences {
    private final SharedPreferences values;

    PluginPreferences(Context context) {
        values = context.getSharedPreferences("ble-heart-rate", Context.MODE_PRIVATE);
    }

    String address() { return values.getString("address", ""); }
    String deviceName() { return values.getString("deviceName", ""); }
    String alias() { return values.getString("alias", "Heart rate"); }
    String operatorIdentity() { return values.getString("operatorIdentity", ""); }
    long staleAfterMs() { return values.getLong("staleAfterMs", 30_000L); }
    boolean sharingEnabled() { return values.getBoolean("sharingEnabled", false); }
    String destination() { return values.getString("destination", ""); }
    long sendIntervalMs() { return values.getLong("sendIntervalMs", 30_000L); }

    void setDevice(String address, String name) {
        values.edit().putString("address", address).putString("deviceName", name).apply();
    }

    void update(JSONObject request) {
        final long staleSeconds = Math.max(5L, Math.min(600L, request.optLong("staleTimeoutSeconds", 30L)));
        final long intervalSeconds = Math.max(5L, Math.min(3_600L, request.optLong("sendIntervalSeconds", 30L)));
        final boolean sharingEnabled = request.optBoolean("sharingEnabled", false);
        final String destination = request.optString("destination", "").trim().toLowerCase(Locale.ROOT);
        if (sharingEnabled && !destination.matches("[0-9a-f]{32}")) {
            throw new IllegalArgumentException(
                "A 32-character hexadecimal LXMF destination is required when sharing is enabled"
            );
        }
        values.edit()
            .putString("alias", request.optString("alias", alias()).trim())
            .putString("operatorIdentity", request.optString("operatorRnsIdentity", operatorIdentity()).trim())
            .putLong("staleAfterMs", staleSeconds * 1_000L)
            .putBoolean("sharingEnabled", sharingEnabled)
            .putString("destination", destination)
            .putLong("sendIntervalMs", intervalSeconds * 1_000L)
            .apply();
    }

    JSONObject json(String connectionState) throws Exception {
        return new JSONObject()
            .put("type", "state")
            .put("selectedDevice", address())
            .put("deviceName", deviceName())
            .put("alias", alias())
            .put("operatorRnsIdentity", operatorIdentity())
            .put("connectionStatus", connectionState)
            .put("staleTimeoutSeconds", staleAfterMs() / 1_000L)
            .put("sharingEnabled", sharingEnabled())
            .put("destination", destination())
            .put("deliveryMode", "Auto")
            .put("sendIntervalSeconds", sendIntervalMs() / 1_000L);
    }
}
