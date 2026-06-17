package network.reticulum.emergency;

import org.json.JSONException;
import org.json.JSONObject;

final class RemWatchStatusServerSettings {
    static final boolean DEFAULT_ENABLED = true;
    static final int DEFAULT_PORT = 29_863;
    static final int MIN_PORT = 1_024;
    static final int MAX_PORT = 65_535;

    final boolean enabled;
    final int port;

    private RemWatchStatusServerSettings(boolean enabled, int port) {
        this.enabled = enabled;
        this.port = port;
    }

    static RemWatchStatusServerSettings normalize(boolean enabled, int port) {
        final int normalizedPort = isValidPort(port) ? port : DEFAULT_PORT;
        return new RemWatchStatusServerSettings(enabled, normalizedPort);
    }

    static RemWatchStatusServerSettings defaults() {
        return normalize(DEFAULT_ENABLED, DEFAULT_PORT);
    }

    static boolean isValidPort(int port) {
        return port >= MIN_PORT && port <= MAX_PORT;
    }

    String url() {
        return "http://localhost:" + port + "/info.json";
    }

    JSONObject toJson(boolean running, String bindError) throws JSONException {
        final JSONObject payload = new JSONObject();
        payload.put("enabled", enabled);
        payload.put("port", port);
        payload.put("url", url());
        payload.put("currentUrl", url());
        payload.put("running", running);
        payload.put("bindError", bindError == null ? "" : bindError);
        return payload;
    }
}
