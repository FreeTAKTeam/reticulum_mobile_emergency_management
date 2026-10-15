package network.reticulum.emergency.plugin.api;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import org.json.JSONException;
import org.json.JSONObject;
import org.junit.Test;

public final class PluginProtocolTest {
    @Test
    public void negotiatesTheLowerMinorAndRejectsNewerPlugins() {
        assertEquals(1, PluginProtocol.negotiateApiMinor(PluginProtocol.API_MINOR, 1, 1));
        assertEquals(3, PluginProtocol.negotiateApiMinor(PluginProtocol.API_MINOR, 1, 3));
        assertEquals(2, PluginProtocol.negotiateApiMinor(2, 1, 2));
        assertEquals(-1, PluginProtocol.negotiateApiMinor(2, 1, 3));
        assertEquals(-1, PluginProtocol.negotiateApiMinor(PluginProtocol.API_MINOR, 2, 0));
        assertEquals(-1, PluginProtocol.negotiateApiMinor(PluginProtocol.API_MINOR, 1, -1));
    }

    @Test
    public void enforcesTheBinderMessageLimitInUtf8Bytes() throws JSONException {
        final String ascii = "x".repeat(PluginProtocol.MAX_JSON_BYTES);
        assertTrue(PluginProtocol.fitsMessageLimit(ascii));
        assertFalse(PluginProtocol.fitsMessageLimit(ascii + "x"));
        // Two bytes per char in UTF-8: half as many chars already reach the limit.
        assertFalse(PluginProtocol.fitsMessageLimit("é".repeat(PluginProtocol.MAX_JSON_BYTES / 2 + 1)));
        assertFalse(PluginProtocol.fitsMessageLimit(""));
        assertFalse(PluginProtocol.fitsMessageLimit(null));
        assertThrows(
            JSONException.class,
            () -> PluginProtocol.request("r", "map.markers.create", new JSONObject().put("notes", ascii))
        );
    }

    @Test
    public void buildsRequestAndEventEnvelopes() throws JSONException {
        final JSONObject request = PluginProtocol.requireEnvelope(
            PluginProtocol.request("req-1", "map.session.list", null)
        );
        assertEquals("map.session.list", request.getString("operation"));
        assertEquals(1, request.getInt("protocolVersion"));
        final JSONObject event = new JSONObject(
            PluginProtocol.hostEvent(
                PluginProtocol.EVENT_MAP_SESSION_STOPPED,
                new JSONObject().put("sessionId", "s").put("reason", "operator")
            )
        );
        assertEquals("map.session.stopped", event.getString("event"));
        assertEquals("operator", event.getJSONObject("payload").getString("reason"));
        assertThrows(JSONException.class, () -> PluginProtocol.requireEnvelope("{\"protocolVersion\":2,\"requestId\":\"x\"}"));
    }
}
