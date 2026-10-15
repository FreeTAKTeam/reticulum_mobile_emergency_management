package network.reticulum.emergency.plugin.api;

import org.json.JSONException;
import org.json.JSONObject;

public final class PluginProtocol {
    public static final int API_MAJOR = 1;
    /** Newest API minor this SDK and the REM host understand. */
    public static final int API_MINOR = 3;
    /** First API minor with the map marker, symbol and observer operations. */
    public static final int MAP_API_MINOR = 2;
    /** First API minor with operator-selected map sessions and their events. */
    public static final int MAP_SESSION_API_MINOR = 3;
    public static final int MAX_JSON_BYTES = 65_536;
    public static final String SERVICE_ACTION = "network.reticulum.emergency.PLUGIN_V1";

    public static final String EVENT_MAP_SESSION_STARTED = "map.session.started";
    public static final String EVENT_MAP_SESSION_OBJECT_UPDATED = "map.session.object.updated";
    public static final String EVENT_MAP_SESSION_OBJECT_REMOVED = "map.session.object.removed";
    public static final String EVENT_MAP_SESSION_STOPPED = "map.session.stopped";

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

    /**
     * The API minor a host and plugin agree on: the lower of the two, or -1
     * when the plugin targets a newer minor than the host understands (the
     * host then treats the plugin as incompatible and never binds it).
     */
    public static int negotiateApiMinor(int hostApiMinor, int pluginApiMajor, int pluginApiMinor) {
        if (pluginApiMajor != API_MAJOR || pluginApiMinor < 0 || pluginApiMinor > hostApiMinor) {
            return -1;
        }
        return pluginApiMinor;
    }

    /** True when the UTF-8 encoding fits one plugin protocol message. */
    public static boolean fitsMessageLimit(String raw) {
        return raw != null
            && !raw.isEmpty()
            && raw.getBytes(java.nio.charset.StandardCharsets.UTF_8).length <= MAX_JSON_BYTES;
    }

    /** Builds a request envelope; throws when it would exceed the 64 KiB limit. */
    public static String request(String requestId, String operation, JSONObject payload)
        throws JSONException {
        final String raw = new JSONObject()
            .put("protocolVersion", API_MAJOR)
            .put("requestId", requestId)
            .put("operation", operation)
            .put("payload", payload == null ? new JSONObject() : payload)
            .toString();
        requireJsonSize(raw, "Plugin request");
        return raw;
    }

    /** Builds a host event envelope delivered through {@code onHostEvent}. */
    public static String hostEvent(String event, JSONObject payload) throws JSONException {
        final String raw = new JSONObject()
            .put("protocolVersion", API_MAJOR)
            .put("event", event)
            .put("payload", payload == null ? new JSONObject() : payload)
            .toString();
        requireJsonSize(raw, "Plugin host event");
        return raw;
    }

    public static void requireJsonSize(String raw, String label) throws JSONException {
        if (!fitsMessageLimit(raw)) {
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
        } catch (JSONException error) {
            android.util.Log.w("REM.PluginProtocol", "Failed to encode structured error response", error);
            return "{\"protocolVersion\":1,\"requestId\":\"\",\"ok\":false}";
        }
    }
}
