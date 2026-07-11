package org.freetakteam.rem.plugin.fixture;

import android.content.SharedPreferences;
import java.util.Collections;
import java.util.Set;
import java.util.UUID;
import network.reticulum.emergency.plugin.api.CallerCertificateVerifier;
import network.reticulum.emergency.plugin.api.IRemPluginConfigurationCallback;
import network.reticulum.emergency.plugin.api.IRemPluginHost;
import network.reticulum.emergency.plugin.api.PluginProtocol;
import network.reticulum.emergency.plugin.api.RemPluginService;
import org.json.JSONObject;

public final class FixturePluginService extends RemPluginService {
    private IRemPluginHost host;

    @Override
    protected Set<String> allowedHostCertificateFingerprints() {
        return CallerCertificateVerifier.packageFingerprints(this, "network.reticulum.emergency");
    }

    @Override
    protected Set<String> allowedHostPackageNames() {
        return Collections.singleton("network.reticulum.emergency");
    }

    @Override
    protected String getDescriptorJson() {
        return "{\"pluginId\":\"org.freetakteam.rem.plugin.fixture\",\"apiMajor\":1,\"apiMinor\":0}";
    }

    @Override
    protected void onPluginStart(IRemPluginHost host, String sessionJson) {
        this.host = host;
        publishFixtureSensor();
    }

    @Override
    protected void onPluginStop(String reason) {
        host = null;
    }

    @Override
    protected void onHostEvent(String eventJson) {}

    @Override
    protected void onHostResponse(String responseJson) {}

    @Override
    protected void onConfigurationRequest(
        String requestJson,
        IRemPluginConfigurationCallback callback
    ) {
        try {
            final JSONObject request = new JSONObject(requestJson);
            final String type = request.optString("type", "");
            final SharedPreferences preferences = getSharedPreferences("fixture", MODE_PRIVATE);
            if ("update".equals(type)) {
                preferences.edit().putString("label", request.optString("label", "Fixture sensor")).apply();
            }
            final JSONObject response = new JSONObject()
                .put("type", "state")
                .put("label", preferences.getString("label", "Fixture sensor"));
            callback.onResponse(response.toString());
        } catch (Exception error) {
            try {
                callback.onResponse(
                    new JSONObject().put("type", "validationError").put("message", error.getMessage()).toString()
                );
            } catch (Exception ignored) {
            }
        }
    }

    private void publishFixtureSensor() {
        if (host == null) {
            return;
        }
        try {
            final JSONObject payload = new JSONObject()
                .put("deviceId", "fixture-device")
                .put("sensorType", "fixture_value")
                .put("displayName", "Fixture sensor")
                .put("value", 1)
                .put("unit", "test")
                .put("timestampMs", System.currentTimeMillis())
                .put("staleAfterMs", 30_000)
                .put("origin", "local");
            host.submitRequest(
                new JSONObject()
                    .put("protocolVersion", PluginProtocol.API_MAJOR)
                    .put("requestId", UUID.randomUUID().toString())
                    .put("operation", "sensor.publish")
                    .put("payload", payload)
                    .toString()
            );
        } catch (Exception ignored) {
        }
    }
}
