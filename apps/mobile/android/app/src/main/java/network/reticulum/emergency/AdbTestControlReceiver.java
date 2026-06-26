package network.reticulum.emergency;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.util.Log;

public final class AdbTestControlReceiver extends BroadcastReceiver {
    public static final String ACTION_ANNOUNCE = "network.reticulum.emergency.action.ADB_ANNOUNCE";
    public static final String ACTION_CONNECT_PEER = "network.reticulum.emergency.action.ADB_CONNECT_PEER";
    public static final String ACTION_DISCONNECT_PEER = "network.reticulum.emergency.action.ADB_DISCONNECT_PEER";
    public static final String ACTION_STATUS = "network.reticulum.emergency.action.ADB_STATUS";
    public static final String EXTRA_DESTINATION_HEX = "destinationHex";

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
            } else if (ACTION_STATUS.equals(action)) {
                Log.i(TAG, "status " + ReticulumBridge.getStatusJson());
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

    private static void logResult(String label, int result) {
        final String outcome = result == 0 ? "ok" : "failed";
        Log.i(TAG, label + " outcome=" + outcome + " result=" + result);
    }
}
