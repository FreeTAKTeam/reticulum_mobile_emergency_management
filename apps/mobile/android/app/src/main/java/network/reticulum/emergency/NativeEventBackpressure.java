package network.reticulum.emergency;

import com.getcapacitor.JSObject;

import java.util.Locale;

final class NativeEventBackpressure {
    private static final String[] NOISY_LOG_PATTERNS = new String[] {
        "[tp-diag] inbound_packet",
        "[iface][rx]",
        "[announceReceived]",
        "[packetReceived]",
        "[link][maintain]",
        "[lxmf][events] link activation retry",
        "[lxmf][queue]",
        "[lxmf][events][sdk] attempting send",
        "[lxmf][mission] resolved send",
        " is now reachable over ",
        "repeat link request",
        "RNode BLE packet serialize failed",
        "close /",
    };

    private NativeEventBackpressure() {
    }

    static boolean shouldDispatchToUi(String eventName, JSObject payload) {
        if ("log".equals(eventName)) {
            return shouldDispatchLog(payload);
        }
        if ("announceReceived".equals(eventName)) {
            return isEmergencyLxmfAnnounce(payload);
        }
        if ("packetReceived".equals(eventName)) {
            return hasMissionFields(payload);
        }
        return true;
    }

    private static boolean shouldDispatchLog(JSObject payload) {
        final String level = payload == null ? "" : payload.getString("level", "");
        final String message = payload == null ? "" : payload.getString("message", "");
        if ("Error".equals(level)) {
            return true;
        }
        for (String pattern : NOISY_LOG_PATTERNS) {
            if (message.contains(pattern)) {
                return false;
            }
        }
        return true;
    }

    private static boolean isEmergencyLxmfAnnounce(JSObject payload) {
        if (payload == null) {
            return false;
        }
        final String destinationKind = payload.getString("destinationKind", "");
        final String announceClass = payload.getString("announceClass", "");
        if (!"lxmf_delivery".equals(destinationKind) && !"LxmfDelivery".equals(announceClass)) {
            return false;
        }
        final String appData = payload.getString("appData", "").toLowerCase(Locale.US);
        return appData.contains("r3akt")
            || appData.contains("emergencymessages")
            || (appData.contains("emergency") && appData.contains("telemetry"));
    }

    private static boolean hasMissionFields(JSObject payload) {
        if (payload == null) {
            return false;
        }
        return payload.getString("fieldsBase64", "").trim().length() > 0;
    }
}
