package network.reticulum.emergency.plugin.api;

import org.json.JSONException;
import org.json.JSONObject;

public final class PluginProtocol {
    public static final int API_MAJOR = 1;
    public static final int API_MINOR = 1;
    public static final int MAX_JSON_BYTES = 65_536;
    public static final String SERVICE_ACTION = "network.reticulum.emergency.PLUGIN_V1";

    private PluginProtocol() {}

    public static JSONObject requireEnvelope(String raw) throws JSONException {
        requireJsonSize(raw, "Plugin protocol message");
        final JSONObject envelope = new JSONObject(raw);
        if (envelope.optInt("protocolVersion", -1) != API_MAJOR) {
            throw new JSONException("Unsupported plugin protocol version");
        }
        if (envelope.optString("requestId", "").trim().isEmpty()) {
            throw new JSONException("Plugin requestId is required");
        }
        return envelope;
    }

    public static void requireJsonSize(String raw, String label) throws JSONException {
        if (raw == null
            || raw.isEmpty()
            || raw.getBytes(java.nio.charset.StandardCharsets.UTF_8).length > MAX_JSON_BYTES) {
            throw new JSONException(label + " is empty or too large");
        }
    }

    public static String errorResponse(String requestId, String code, String message) {
        try {
            return new JSONObject()
                .put("protocolVersion", API_MAJOR)
                .put("requestId", requestId == null ? "" : requestId)
                .put("ok", false)
                .put("error", new JSONObject().put("code", code).put("message", message))
                .toString();
        } catch (JSONException ignored) {
            return "{\"protocolVersion\":1,\"requestId\":\"\",\"ok\":false}";
        }
    }
}
