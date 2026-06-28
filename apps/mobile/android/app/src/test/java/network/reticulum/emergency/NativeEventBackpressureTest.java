package network.reticulum.emergency;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import com.getcapacitor.JSObject;

import org.junit.Test;

public class NativeEventBackpressureTest {
    @Test
    public void dropsPacketDiagnosticsBeforeWebViewDispatch() {
        final JSObject payload = new JSObject();
        payload.put("level", "Info");
        payload.put("message", "[tp-diag] inbound_packet node=S8 iface=/abc/ type=Data hash=123");

        assertFalse(NativeEventBackpressure.shouldDispatchToUi("log", payload));
    }

    @Test
    public void dropsNonEmergencyAnnouncesBeforeWebViewDispatch() {
        final JSObject payload = new JSObject();
        payload.put("announceClass", "Other");
        payload.put("destinationKind", "other");
        payload.put("appData", "");

        assertFalse(NativeEventBackpressure.shouldDispatchToUi("announceReceived", payload));
    }

    @Test
    public void keepsEmergencyLxmfAnnouncesForPeerDiscovery() {
        final JSObject payload = new JSObject();
        payload.put("announceClass", "LxmfDelivery");
        payload.put("destinationKind", "lxmf_delivery");
        payload.put("appData", "R3AKT,EmergencyMessages,Telemetry;name=S8");

        assertTrue(NativeEventBackpressure.shouldDispatchToUi("announceReceived", payload));
    }

    @Test
    public void dropsPacketsWithoutMissionFieldsBeforeWebViewDispatch() {
        final JSObject payload = new JSObject();
        payload.put("sourceHex", "abc");
        payload.put("destinationHex", "def");
        payload.put("bytesBase64", "AAECAw==");

        assertFalse(NativeEventBackpressure.shouldDispatchToUi("packetReceived", payload));
    }

    @Test
    public void keepsPacketsWithMissionFieldsForBootstrapListeners() {
        final JSObject payload = new JSObject();
        payload.put("sourceHex", "abc");
        payload.put("destinationHex", "def");
        payload.put("fieldsBase64", "gahtYXJrZXTDpGRlbW8=");

        assertTrue(NativeEventBackpressure.shouldDispatchToUi("packetReceived", payload));
    }

    @Test
    public void dropsLinkActivationRetriesBeforeWebViewDispatch() {
        final JSObject payload = new JSObject();
        payload.put("level", "Info");
        payload.put(
            "message",
            "[lxmf][events] link activation retry destination=a1c8126d7cb806e6bde086d582b6cb0d attempt=2 timeout_ms=20000 reason=timeout"
        );

        assertFalse(NativeEventBackpressure.shouldDispatchToUi("log", payload));
    }

    @Test
    public void keepsReadinessErrorsForUiState() {
        final JSObject payload = new JSObject();
        payload.put("level", "Error");
        payload.put("message", "transport startup failed: no reachable Reticulum TCP interface");

        assertTrue(NativeEventBackpressure.shouldDispatchToUi("log", payload));
    }
}
