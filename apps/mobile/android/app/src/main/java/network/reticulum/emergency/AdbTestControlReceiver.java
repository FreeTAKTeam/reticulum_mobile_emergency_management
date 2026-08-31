package network.reticulum.emergency;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.util.Base64;
import android.util.Log;

import org.json.JSONArray;
import org.json.JSONObject;

import java.nio.charset.StandardCharsets;

public final class AdbTestControlReceiver extends BroadcastReceiver {
    public static final String ACTION_ANNOUNCE = "network.reticulum.emergency.action.ADB_ANNOUNCE";
    public static final String ACTION_CONNECT_PEER = "network.reticulum.emergency.action.ADB_CONNECT_PEER";
    public static final String ACTION_DISCONNECT_PEER = "network.reticulum.emergency.action.ADB_DISCONNECT_PEER";
    public static final String ACTION_APP_SETTINGS = "network.reticulum.emergency.action.ADB_APP_SETTINGS";
    public static final String ACTION_EAMS = "network.reticulum.emergency.action.ADB_EAMS";
    public static final String ACTION_EVENTS = "network.reticulum.emergency.action.ADB_EVENTS";
    public static final String ACTION_STATUS = "network.reticulum.emergency.action.ADB_STATUS";
    public static final String ACTION_SEND_LXMF = "network.reticulum.emergency.action.ADB_SEND_LXMF";
    public static final String ACTION_UPSERT_EVENT = "network.reticulum.emergency.action.ADB_UPSERT_EVENT";
    public static final String ACTION_ANNOUNCES = "network.reticulum.emergency.action.ADB_ANNOUNCES";
    public static final String ACTION_MESSAGES = "network.reticulum.emergency.action.ADB_MESSAGES";
    public static final String ACTION_ASSERT_ANNOUNCE = "network.reticulum.emergency.action.ADB_ASSERT_ANNOUNCE";
    public static final String ACTION_ASSERT_MESSAGE = "network.reticulum.emergency.action.ADB_ASSERT_MESSAGE";
    public static final String ACTION_ASSERT_EVENT = "network.reticulum.emergency.action.ADB_ASSERT_EVENT";
    public static final String EXTRA_DESTINATION_HEX = "destinationHex";
    public static final String EXTRA_PAYLOAD_JSON = "payloadJson";
    public static final String EXTRA_PAYLOAD_BASE64 = "payloadBase64";

    private static final String TAG = "ReticulumAdbTest";

    @Override
    public void onReceive(Context context, Intent intent) {
        if (!BuildConfig.ENABLE_ADB_TEST_CONTROL) {
            Log.w(TAG, "rejected action: adb test control is disabled");
            return;
        }
        if (intent == null || intent.getAction() == null) {
            Log.w(TAG, "rejected action: missing action");
            return;
        }

        final Intent receivedIntent = new Intent(intent);
        new Thread(() -> handleAction(receivedIntent), "rem-adb-test-control").start();
    }

    private static void handleAction(Intent intent) {
        final String action = intent.getAction();
        try {
            if (ACTION_ANNOUNCE.equals(action)) {
                logResult("announce", ReticulumBridge.announceNow());
            } else if (ACTION_CONNECT_PEER.equals(action)) {
                final String destinationHex = requireDestination(intent);
                logResult("connect destination=" + destinationHex, ReticulumBridge.connectPeer(destinationHex));
            } else if (ACTION_DISCONNECT_PEER.equals(action)) {
                final String destinationHex = requireDestination(intent);
                logResult("disconnect destination=" + destinationHex, ReticulumBridge.disconnectPeer(destinationHex));
            } else if (ACTION_APP_SETTINGS.equals(action)) {
                Log.i(TAG, "appSettings " + ReticulumBridge.getAppSettingsJson());
            } else if (ACTION_EAMS.equals(action)) {
                Log.i(TAG, "eams " + ReticulumBridge.getEamsJson());
            } else if (ACTION_EVENTS.equals(action)) {
                Log.i(TAG, "events " + ReticulumBridge.getEventsJson());
            } else if (ACTION_STATUS.equals(action)) {
                Log.i(TAG, "status " + ReticulumBridge.getStatusJson());
                Log.i(TAG, "rnodeTransport " + RNodeAndroidTransportManager.status());
            } else if (ACTION_SEND_LXMF.equals(action)) {
                logJsonResult("sendLxmf", ReticulumBridge.sendLxmfJson(requirePayload(intent)));
            } else if (ACTION_UPSERT_EVENT.equals(action)) {
                logResult("upsertEvent", ReticulumBridge.upsertEventJson(requirePayload(intent)));
            } else if (ACTION_ANNOUNCES.equals(action)) {
                Log.i(TAG, "announces " + ReticulumBridge.listAnnouncesJson());
            } else if (ACTION_MESSAGES.equals(action)) {
                Log.i(TAG, "messages " + ReticulumBridge.listMessagesJson(requirePayload(intent)));
            } else if (ACTION_ASSERT_ANNOUNCE.equals(action)) {
                assertAnnounce(requirePayload(intent));
            } else if (ACTION_ASSERT_MESSAGE.equals(action)) {
                assertMessage(requirePayload(intent));
            } else if (ACTION_ASSERT_EVENT.equals(action)) {
                assertEvent(requirePayload(intent));
            } else {
                Log.w(TAG, "rejected action: unsupported action=" + action);
            }
        } catch (Exception ex) {
            Log.e(TAG, "action failed action=" + action + " reason=" + ex.getMessage(), ex);
        }
    }

    private static String requireDestination(Intent intent) {
        final String destinationHex = intent.getStringExtra(EXTRA_DESTINATION_HEX);
        if (destinationHex == null || destinationHex.trim().isEmpty()) {
            throw new IllegalArgumentException("missing destinationHex");
        }
        return destinationHex.trim();
    }

    private static String requirePayload(Intent intent) {
        final String payloadJson = intent.getStringExtra(EXTRA_PAYLOAD_JSON);
        if (payloadJson != null && !payloadJson.trim().isEmpty()) {
            return payloadJson.trim();
        }
        final String payloadBase64 = intent.getStringExtra(EXTRA_PAYLOAD_BASE64);
        if (payloadBase64 == null || payloadBase64.trim().isEmpty()) {
            throw new IllegalArgumentException("missing payloadJson or payloadBase64");
        }
        try {
            return new String(
                Base64.decode(payloadBase64.trim(), Base64.NO_WRAP),
                StandardCharsets.UTF_8
            );
        } catch (IllegalArgumentException error) {
            throw new IllegalArgumentException("payloadBase64 is invalid", error);
        }
    }

    private static void logResult(String label, int result) {
        final String outcome = result == 0 ? "ok" : "failed";
        Log.i(TAG, label + " outcome=" + outcome + " result=" + result);
        if (result != 0) {
            Log.e(TAG, label + " error=" + ReticulumBridge.takeLastErrorJson());
        }
    }

    private static void logJsonResult(String label, String result) {
        if (result == null) {
            Log.e(TAG, label + " outcome=failed error=" + ReticulumBridge.takeLastErrorJson());
            return;
        }
        Log.i(TAG, label + " outcome=ok result=" + result);
    }

    private static void assertAnnounce(String payload) throws Exception {
        final JSONObject request = new JSONObject(payload);
        final String id = request.getString("assertionId");
        final String destination = request.getString("destinationHex");
        final long receivedAfterMs = request.getLong("receivedAfterMs");
        final JSONArray items = new JSONObject(ReticulumBridge.listAnnouncesJson())
            .getJSONArray("items");
        boolean found = false;
        for (int index = 0; index < items.length(); index++) {
            final JSONObject item = items.getJSONObject(index);
            if (destination.equals(item.optString("destination_hex"))
                && item.optLong("received_at_ms", 0L) >= receivedAfterMs) {
                found = true;
                break;
            }
        }
        logAssertion("assertAnnounce", id, found);
    }

    private static void assertMessage(String payload) throws Exception {
        final JSONObject request = new JSONObject(payload);
        final String id = request.getString("assertionId");
        final String expectedBody = request.getString("expectedBody");
        final boolean prefix = request.optBoolean("prefix", false);
        final JSONArray items = new JSONObject(ReticulumBridge.listMessagesJson("{}"))
            .getJSONArray("items");
        boolean found = false;
        for (int index = 0; index < items.length(); index++) {
            final JSONObject item = items.getJSONObject(index);
            final String body = item.optString("bodyUtf8");
            final boolean bodyMatches = prefix ? body.startsWith(expectedBody) : body.equals(expectedBody);
            if (bodyMatches && "Inbound".equals(item.optString("direction"))) {
                found = true;
                break;
            }
        }
        logAssertion("assertMessage", id, found);
    }

    private static void assertEvent(String payload) throws Exception {
        final JSONObject request = new JSONObject(payload);
        final String id = request.getString("assertionId");
        final String expectedUid = request.getString("eventUid");
        final JSONArray items = new JSONObject(ReticulumBridge.getEventsJson()).getJSONArray("items");
        boolean found = false;
        for (int index = 0; index < items.length(); index++) {
            final JSONObject args = items.getJSONObject(index).optJSONObject("args");
            if (args != null && expectedUid.equals(args.optString("entry_uid"))) {
                found = true;
                break;
            }
        }
        logAssertion("assertEvent", id, found);
    }

    private static void logAssertion(String kind, String id, boolean found) {
        Log.i(TAG, kind + " id=" + id + " outcome=" + (found ? "ok" : "missing"));
    }
}
