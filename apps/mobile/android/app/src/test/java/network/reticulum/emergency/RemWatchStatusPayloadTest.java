package network.reticulum.emergency;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import org.json.JSONObject;
import org.junit.Test;

public class RemWatchStatusPayloadTest {
    private static final long NOW_MS = 1_760_000_120_000L;
    private static final long LAST_SYNC_MS = 1_760_000_000_000L;

    @Test
    public void buildsConnectedStatusFromRuntimeJson() throws Exception {
        final JSONObject payload = new JSONObject(RemWatchStatusPayload.build(
            "{\"running\":true,\"name\":\"OP ALPHA-1\"}",
            "{\"running\":true,\"eventCount\":3,\"updatedAtMs\":" + LAST_SYNC_MS + "}",
            "{\"activeTotal\":2,\"messages\":[{\"callsign\":\"OP ALPHA-1\",\"overallBand\":\"Green\"}],\"statusMetrics\":[{\"band\":\"Yellow\"}]}",
            "{\"items\":[]}",
            "{\"items\":[]}",
            NOW_MS
        ));

        assertEquals("rem.watch.status", payload.getString("type"));
        assertEquals(1, payload.getInt("version"));
        assertEquals("CONNECTED", payload.getString("connection_state"));
        assertEquals("OP ALPHA-1", payload.getString("operator_name"));
        assertEquals("Green", payload.getString("operator_eam"));
        assertEquals("Yellow", payload.getString("team_status"));
        assertEquals(3, payload.getInt("active_events"));
        assertEquals(LAST_SYNC_MS, payload.getLong("last_sync_epoch_ms"));
        assertEquals(120, payload.getInt("last_sync_age_seconds"));
        assertEquals("HIGH", payload.getString("highest_priority"));
        assertEquals("NORMAL", payload.getString("alert_state"));
        assertFalse(payload.has("latest_event"));
    }

    @Test
    public void buildsOfflineStatusWhenNodeStopped() throws Exception {
        final JSONObject payload = new JSONObject(RemWatchStatusPayload.build(
            "{\"running\":false,\"name\":\"OP ALPHA-1\"}",
            "{\"running\":false,\"eventCount\":0,\"updatedAtMs\":" + LAST_SYNC_MS + "}",
            "{}",
            "{\"items\":[]}",
            "{}",
            NOW_MS
        ));

        assertEquals("OFFLINE", payload.getString("connection_state"));
        assertEquals("NORMAL", payload.getString("highest_priority"));
        assertEquals("NORMAL", payload.getString("alert_state"));
    }

    @Test
    public void preservesOrangeReadinessBands() throws Exception {
        final JSONObject payload = new JSONObject(RemWatchStatusPayload.build(
            "{\"running\":true,\"name\":\"OP ALPHA-1\"}",
            "{\"running\":true,\"eventCount\":0,\"updatedAtMs\":" + LAST_SYNC_MS + "}",
            "{\"activeTotal\":2,\"messages\":[{\"callsign\":\"OP ALPHA-1\",\"overallBand\":\"Orange\"}],\"statusMetrics\":[{\"band\":\"Yellow\"},{\"band\":\"Orange\"}]}",
            "{\"items\":[]}",
            "{\"items\":[]}",
            NOW_MS
        ));

        assertEquals("Orange", payload.getString("operator_eam"));
        assertEquals("Orange", payload.getString("team_status"));
        assertEquals("HIGH", payload.getString("highest_priority"));
    }

    @Test
    public void buildsErrorStatusWhenRuntimeReportsLastError() throws Exception {
        final JSONObject payload = new JSONObject(RemWatchStatusPayload.build(
            "{\"running\":false,\"name\":\"OP ALPHA-1\",\"lastError\":\"bind failed\"}",
            "{\"running\":false,\"eventCount\":0,\"updatedAtMs\":" + LAST_SYNC_MS + "}",
            "{}",
            "{\"items\":[]}",
            "{}",
            NOW_MS
        ));

        assertEquals("ERROR", payload.getString("connection_state"));
        assertEquals("ERROR", payload.getString("highest_priority"));
        assertEquals("ERROR", payload.getString("alert_state"));
    }

    @Test
    public void mapsLatestEventFromEventProjection() throws Exception {
        final JSONObject payload = new JSONObject(RemWatchStatusPayload.build(
            "{\"running\":true,\"name\":\"OP ALPHA-1\"}",
            "{\"running\":true,\"eventCount\":1,\"updatedAtMs\":" + LAST_SYNC_MS + "}",
            "{}",
            "{\"items\":[{\"command_type\":\"event.upsert\",\"timestamp\":\"2026-06-16T12:34:56Z\",\"source\":{\"display_name\":\"Poco\"},\"args\":{\"content\":\"help coming\",\"callsign\":\"Poco\",\"server_time\":\"2026-06-16T12:34:56Z\",\"keywords\":[\"response\"]}}]}",
            "{}",
            NOW_MS
        ));

        final JSONObject latestEvent = payload.getJSONObject("latest_event");
        assertEquals("HIGH", latestEvent.getString("severity"));
        assertEquals("RESPONSE", latestEvent.getString("category"));
        assertEquals("help coming", latestEvent.getString("title"));
        assertEquals("Poco", latestEvent.getString("source"));
        assertEquals("2026-06-16T12:34:56Z", latestEvent.getString("time"));
        assertEquals("HIGH", payload.getString("highest_priority"));
    }

    @Test
    public void validatesWatchStatusServerSettingsPortRange() {
        final RemWatchStatusServerSettings defaults = RemWatchStatusServerSettings.normalize(true, 0);
        assertTrue(defaults.enabled);
        assertEquals(29_863, defaults.port);

        final RemWatchStatusServerSettings belowRange = RemWatchStatusServerSettings.normalize(false, 1023);
        assertFalse(belowRange.enabled);
        assertEquals(29_863, belowRange.port);

        final RemWatchStatusServerSettings custom = RemWatchStatusServerSettings.normalize(true, 49_999);
        assertTrue(custom.enabled);
        assertEquals(49_999, custom.port);
    }
}
