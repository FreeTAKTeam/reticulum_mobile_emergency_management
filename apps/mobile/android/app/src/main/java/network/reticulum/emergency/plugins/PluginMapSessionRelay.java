package network.reticulum.emergency.plugins;

import network.reticulum.emergency.plugin.api.PluginProtocol;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Moves plugin map session events from the native outbox to plugin Binders. The native runtime
 * decides which plugin may see which object; this relay only delivers what it was handed, and
 * drops events for plugins whose connection is gone.
 */
final class PluginMapSessionRelay {
    static final int MAX_DRAIN_ROUNDS = 8;

    interface Source {
        /** One native drain: {@code {"items":[{"pluginId":...,"event":{...}}]}} or null. */
        String drain();
    }

    interface Sink {
        /** Delivers one event; returns false when the plugin has no live connection. */
        boolean deliver(String pluginId, String eventJson);
    }

    private PluginMapSessionRelay() {}

    /** Drains until the outbox is empty (bounded); returns the number of delivered events. */
    static int deliverPending(Source source, Sink sink) throws JSONException {
        int delivered = 0;
        for (int round = 0; round < MAX_DRAIN_ROUNDS; round++) {
            final String raw = source.drain();
            if (raw == null || raw.isEmpty()) {
                return delivered;
            }
            final JSONArray items = new JSONObject(raw).optJSONArray("items");
            if (items == null || items.length() == 0) {
                return delivered;
            }
            for (int index = 0; index < items.length(); index++) {
                final JSONObject item = items.getJSONObject(index);
                final JSONObject event = item.optJSONObject("event");
                final String pluginId = item.optString("pluginId", "");
                if (event == null || pluginId.isEmpty()) {
                    continue;
                }
                final String eventJson = event.toString();
                if (PluginProtocol.fitsMessageLimit(eventJson) && sink.deliver(pluginId, eventJson)) {
                    delivered += 1;
                }
            }
        }
        return delivered;
    }

    /** Operator stop from the REM UI: the plugin is told why its session ended. */
    static String operatorStopRequest(String sessionId) throws JSONException {
        return new JSONObject()
            .put("sessionId", sessionId)
            .put("reason", "operator")
            .put("notify", true)
            .toString();
    }

    /**
     * Ends every session of a plugin whose connection is gone (Binder death, unbind, node stop).
     * No event is queued: there is no live plugin to receive it.
     */
    static String connectionLostStopRequest(String pluginId, String reason) throws JSONException {
        return new JSONObject()
            .put("pluginId", pluginId)
            .put("reason", reason)
            .put("notify", false)
            .toString();
    }
}
