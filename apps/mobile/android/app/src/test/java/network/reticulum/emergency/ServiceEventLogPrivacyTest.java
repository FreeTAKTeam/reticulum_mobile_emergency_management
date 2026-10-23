package network.reticulum.emergency;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;

import com.getcapacitor.JSObject;

import org.json.JSONObject;
import org.junit.Test;

public class ServiceEventLogPrivacyTest {
    @Test
    public void receivedPacketLogOmitsPositionFieldsAndPreservesTheEvent() throws Exception {
        final JSObject payload = new JSObject();
        payload.put("sourceHex", "source");
        payload.put("destinationHex", "destination");
        payload.put("bytesBase64", "VA==");
        payload.put("fieldsBase64", "reversible-position-fields");
        payload.put("latitude", 48.123456);
        final String original = payload.toString();

        final JSONObject logged = new JSONObject(
            ServiceEventCoordinator.eventPayloadForLog("packetReceived", payload)
        );

        assertEquals(2, logged.length());
        assertEquals("source", logged.getString("sourceHex"));
        assertEquals("destination", logged.getString("destinationHex"));
        assertFalse(logged.has("fieldsBase64"));
        assertFalse(logged.has("bytesBase64"));
        assertFalse(logged.has("latitude"));
        assertEquals(original, payload.toString());
    }

    @Test
    public void sentPacketLogKeepsOutcomeWithoutEncodedContent() throws Exception {
        final JSObject payload = new JSObject();
        payload.put("destinationHex", "destination");
        payload.put("bytesBase64", "encoded-position-payload");
        payload.put("outcome", "Sent");

        final JSONObject logged = new JSONObject(
            ServiceEventCoordinator.eventPayloadForLog("packetSent", payload)
        );

        assertEquals(2, logged.length());
        assertEquals("destination", logged.getString("destinationHex"));
        assertEquals("Sent", logged.getString("outcome"));
        assertFalse(logged.has("bytesBase64"));
    }

    @Test
    public void applicationDeliveryLogPreservesCorrelationEvidence() {
        final JSObject payload = new JSObject();
        payload.put("messageIdHex", "message-id");
        payload.put("commandId", "command-id");
        payload.put("applicationAckState", "Accepted");
        payload.put("representation", "Packet");

        assertEquals(
            payload.toString(),
            ServiceEventCoordinator.eventPayloadForLog("lxmfDelivery", payload)
        );
    }
}
