package network.reticulum.emergency.plugins;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.List;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;
import org.junit.Test;

public final class PluginMapSessionRelayTest {
    private static JSONObject item(String pluginId, String event, String notes) throws JSONException {
        return new JSONObject()
            .put("pluginId", pluginId)
            .put(
                "event",
                new JSONObject()
                    .put("protocolVersion", 1)
                    .put("event", event)
                    .put("payload", new JSONObject().put("sessionId", "s-1").put("notes", notes))
            );
    }

    private static String batch(JSONObject... items) throws JSONException {
        final JSONArray array = new JSONArray();
        for (JSONObject item : items) {
            array.put(item);
        }
        return new JSONObject().put("items", array).toString();
    }

    @Test
    public void deliversToLivePluginsAndDropsEventsForDeadConnections() throws JSONException {
        final Deque<String> outbox = new ArrayDeque<>();
        outbox.add(batch(
            item("live", "map.session.started", ""),
            item("dead", "map.session.object.updated", ""),
            item("live", "map.session.object.removed", "")
        ));
        outbox.add(batch(item("live", "map.session.stopped", "")));
        final List<String> delivered = new ArrayList<>();
        final Set<String> live = Set.of("live");

        final int count = PluginMapSessionRelay.deliverPending(outbox::poll, (pluginId, eventJson) -> {
            if (!live.contains(pluginId)) {
                return false;
            }
            try {
                delivered.add(new JSONObject(eventJson).getString("event"));
            } catch (JSONException error) {
                throw new AssertionError(error);
            }
            return true;
        });

        assertEquals(3, count);
        assertEquals(
            List.of("map.session.started", "map.session.object.removed", "map.session.stopped"),
            delivered
        );
        assertTrue(outbox.isEmpty());
    }

    @Test
    public void neverForwardsAnEventAboveTheBinderLimit() throws JSONException {
        final Deque<String> outbox = new ArrayDeque<>();
        outbox.add(batch(item("live", "map.session.object.updated", "x".repeat(70_000))));
        final List<String> delivered = new ArrayList<>();
        PluginMapSessionRelay.deliverPending(outbox::poll, (pluginId, eventJson) -> delivered.add(eventJson));
        assertTrue(delivered.isEmpty());
    }

    @Test
    public void drainingIsBoundedPerCall() throws JSONException {
        final String endless = batch(item("live", "map.session.object.updated", ""));
        final int[] drains = {0};
        PluginMapSessionRelay.deliverPending(() -> {
            drains[0] += 1;
            return endless;
        }, (pluginId, eventJson) -> true);
        assertEquals(PluginMapSessionRelay.MAX_DRAIN_ROUNDS, drains[0]);
    }

    @Test
    public void connectionLossEndsSessionsWithoutNotifyingTheDeadPlugin() throws JSONException {
        final JSONObject lost = new JSONObject(
            PluginMapSessionRelay.connectionLostStopRequest("org.example.plugin", "failed")
        );
        assertEquals("org.example.plugin", lost.getString("pluginId"));
        assertFalse(lost.getBoolean("notify"));
        assertFalse(lost.has("sessionId"));

        final JSONObject operator = new JSONObject(PluginMapSessionRelay.operatorStopRequest("s-1"));
        assertEquals("s-1", operator.getString("sessionId"));
        assertEquals("operator", operator.getString("reason"));
        assertTrue(operator.getBoolean("notify"));
    }
}
