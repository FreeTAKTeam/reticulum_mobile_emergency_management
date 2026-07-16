package org.freetakteam.rem.plugin.watchstatus;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import java.net.ServerSocket;
import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.Test;

public class WatchStatusPayloadTest {
    @Test public void preservesOrangeReadinessAndSnapshotShape() throws Exception {
        final JSONObject snapshot = new JSONObject()
            .put("capturedAtMs", 10_000L)
            .put("status", new JSONObject().put("running", true).put("name", "ALPHA"))
            .put("operationalSummary", new JSONObject().put("eventCount", 2).put("updatedAtMs", 9_000L))
            .put("eamReadiness", new JSONObject()
                .put("messages", new JSONArray().put(new JSONObject().put("callsign", "ALPHA").put("overallBand", "Orange")))
                .put("statusMetrics", new JSONArray().put(new JSONObject().put("band", "Orange"))))
            .put("latestPosition", new JSONObject().put("lat", 44.65).put("lon", -63.57));

        final JSONObject payload = new JSONObject(WatchStatusPayload.build(snapshot, 12_000L));
        assertEquals("CONNECTED", payload.getString("connection_state"));
        assertEquals("Orange", payload.getString("operator_eam"));
        assertEquals("Orange", payload.getString("team_status"));
        assertEquals("HIGH", payload.getString("highest_priority"));
        assertEquals(2, payload.getInt("active_events"));
        assertTrue(payload.has("position"));
    }

    @Test public void staleSnapshotErrorUsesValidWatchContract() throws Exception {
        final JSONObject payload = new JSONObject(WatchStatusPayload.error("stale", 50_000L));
        assertEquals("rem.watch.status", payload.getString("type"));
        assertEquals("ERROR", payload.getString("connection_state"));
        assertFalse(payload.has("position"));
    }

    @Test public void validatesPortRangeAndRequestPaths() {
        assertFalse(WatchStatusSettings.isValidPort(1023));
        assertTrue(WatchStatusSettings.isValidPort(29_863));
        assertEquals(29_863, WatchStatusSettings.normalizePort(70_000));
        assertEquals("/health", WatchStatusServer.requestPath("GET /health HTTP/1.1"));
        assertEquals("", WatchStatusServer.requestPath("POST /health HTTP/1.1"));
    }

    @Test public void correlatesOnlyThePendingHostResponse() throws Exception {
        assertTrue(WatchStatusPluginService.responseMatches(
            "request-1",
            new JSONObject().put("requestId", "request-1")
        ));
        assertFalse(WatchStatusPluginService.responseMatches(
            "request-1",
            new JSONObject().put("requestId", "request-2")
        ));
        assertFalse(WatchStatusPluginService.responseMatches("", new JSONObject()));
    }

    @Test public void stoppingServerReleasesLifecycleResources() throws Exception {
        final int port;
        try (ServerSocket probe = new ServerSocket(0)) {
            port = probe.getLocalPort();
        }
        final WatchStatusServer server = new WatchStatusServer();
        server.apply(true, port, () -> "{}");
        assertTrue(server.isRunning());
        server.stop();
        assertFalse(server.isRunning());
        try (ServerSocket rebound = new ServerSocket(port)) {
            assertTrue(rebound.isBound());
        }
    }
}
