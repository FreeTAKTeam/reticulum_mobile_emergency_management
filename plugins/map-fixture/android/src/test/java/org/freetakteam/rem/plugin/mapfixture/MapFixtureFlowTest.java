package org.freetakteam.rem.plugin.mapfixture;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

import java.util.List;
import network.reticulum.emergency.plugin.api.PluginProtocol;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;
import org.junit.Test;

public final class MapFixtureFlowTest {
    private static final String SELECTED = "0123456789abcdef0123456789abcdef";
    private static final String UNSELECTED = "fedcba9876543210fedcba9876543210";

    private int counter;
    private final MapFixtureFlow flow = new MapFixtureFlow(() -> "req-" + (++counter));

    private static JSONObject only(List<String> requests) throws JSONException {
        assertEquals(1, requests.size());
        return new JSONObject(requests.get(0));
    }

    private static String ok(JSONObject request, JSONObject result) throws JSONException {
        return new JSONObject()
            .put("protocolVersion", 1)
            .put("requestId", request.getString("requestId"))
            .put("ok", true)
            .put("result", result)
            .toString();
    }

    private static JSONObject marker(String id, String name) throws JSONException {
        return new JSONObject().put("object_destination_hash", id).put("name", name);
    }

    private static String sessionStarted() throws JSONException {
        return PluginProtocol.hostEvent(
            PluginProtocol.EVENT_MAP_SESSION_STARTED,
            new JSONObject()
                .put("sessionId", "s-1")
                .put("pluginId", "org.freetakteam.rem.plugin.map_fixture")
                .put("objectDestinationHashes", new JSONArray().put(SELECTED))
                .put("objects", new JSONArray().put(marker(SELECTED, "Selected")))
        );
    }

    @Test
    public void createsAMarkerFromTheLocalObserverFix() throws JSONException {
        final JSONObject symbols = only(flow.start("{\"apiMajor\":1,\"apiMinor\":3}"));
        assertEquals("map.symbols.list", symbols.getString("operation"));
        final JSONObject catalog = new JSONObject().put(
            "items",
            new JSONArray().put(new JSONObject().put("id", "fire")).put(new JSONObject().put("id", "hostile"))
        );
        final JSONObject observer = only(flow.onHostResponse(ok(symbols, catalog)));
        assertEquals("map.observer.snapshot", observer.getString("operation"));

        final JSONObject fix = new JSONObject()
            .put("available", true)
            .put("observer", new JSONObject().put("kind", "localDevice").put("identityHex", "aa"))
            .put("lat", 45.0)
            .put("lon", 7.5)
            .put("updatedAtMs", 1_000L);
        final JSONObject create = only(flow.onHostResponse(ok(observer, fix)));
        assertEquals("map.markers.create", create.getString("operation"));
        final JSONObject payload = create.getJSONObject("payload");
        assertEquals("hostile", payload.getString("symbol"));
        assertEquals("fixture-target-1000", payload.getString("idempotencyKey"));
        assertEquals(45.0 + MapFixtureFlow.TARGET_DISTANCE_M / 111_320.0, payload.getDouble("lat"), 1e-9);
        assertEquals(7.5, payload.getDouble("lon"), 0.0);

        flow.onHostResponse(ok(create, new JSONObject().put("marker", marker(SELECTED, "Fixture target"))));
        assertEquals(SELECTED, flow.createdMarkerId());
    }

    @Test
    public void neverTargetsAFixThatIsNotTheLocalDevice() throws JSONException {
        final JSONObject symbols = only(flow.start("{\"apiMajor\":1,\"apiMinor\":3}"));
        final JSONObject observer = only(flow.onHostResponse(ok(
            symbols,
            new JSONObject().put("items", new JSONArray().put(new JSONObject().put("id", "hostile")))
        )));
        final JSONObject peerFix = new JSONObject()
            .put("available", true)
            .put("observer", new JSONObject().put("kind", "peer"))
            .put("lat", 1.0)
            .put("lon", 1.0);
        assertTrue(flow.onHostResponse(ok(observer, peerFix)).isEmpty());
        assertTrue(flow.onHostResponse(ok(observer, new JSONObject().put("available", false))).isEmpty());
    }

    @Test
    public void readsOnlyTheObjectsTheOperatorSelected() throws JSONException {
        flow.start("{\"apiMajor\":1,\"apiMinor\":3}");
        final JSONObject read = only(flow.onHostEvent(sessionStarted()));
        assertEquals("map.session.objects.get", read.getString("operation"));
        assertEquals("s-1", read.getJSONObject("payload").getString("sessionId"));

        final JSONObject result = new JSONObject()
            .put("session", new JSONObject().put("sessionId", "s-1"))
            .put("objects", new JSONArray().put(marker(SELECTED, "Selected")).put(marker(UNSELECTED, "Leak")));
        flow.onHostResponse(ok(read, result));
        assertEquals(List.of(SELECTED), List.copyOf(flow.selectedObjects("s-1").keySet()));
        assertEquals("an unselected object is a contract violation", List.of(UNSELECTED), flow.violations());

        flow.onHostEvent(PluginProtocol.hostEvent(
            PluginProtocol.EVENT_MAP_SESSION_OBJECT_UPDATED,
            new JSONObject().put("sessionId", "s-1").put("object", marker(SELECTED, "Moved"))
        ));
        assertEquals("Moved", flow.selectedObjects("s-1").get(SELECTED).getString("name"));
    }

    @Test
    public void forgetsObjectsWhenTheSessionStopsOrTheObjectIsRemoved() throws JSONException {
        flow.start("{\"apiMajor\":1,\"apiMinor\":3}");
        flow.onHostEvent(sessionStarted());
        flow.onHostEvent(PluginProtocol.hostEvent(
            PluginProtocol.EVENT_MAP_SESSION_OBJECT_UPDATED,
            new JSONObject().put("sessionId", "s-1").put("object", marker(SELECTED, "Selected"))
        ));
        flow.onHostEvent(PluginProtocol.hostEvent(
            PluginProtocol.EVENT_MAP_SESSION_OBJECT_REMOVED,
            new JSONObject().put("sessionId", "s-1").put("objectDestinationHash", SELECTED)
        ));
        assertTrue(flow.selectedObjects("s-1").isEmpty());

        flow.onHostEvent(PluginProtocol.hostEvent(
            PluginProtocol.EVENT_MAP_SESSION_STOPPED,
            new JSONObject().put("sessionId", "s-1").put("reason", "operator")
        ));
        assertTrue(flow.activeSessions().isEmpty());
        flow.onHostEvent(PluginProtocol.hostEvent(
            PluginProtocol.EVENT_MAP_SESSION_OBJECT_UPDATED,
            new JSONObject().put("sessionId", "s-1").put("object", marker(SELECTED, "Late"))
        ));
        assertTrue("events after stop are ignored", flow.selectedObjects("s-1").isEmpty());
    }

    @Test
    public void staysIdleWhenRemNegotiatesAnOlderApi() throws JSONException {
        assertTrue(flow.start("{\"apiMajor\":1,\"apiMinor\":2}").isEmpty());
        assertTrue(flow.onHostEvent(sessionStarted()).isEmpty());
    }
}
