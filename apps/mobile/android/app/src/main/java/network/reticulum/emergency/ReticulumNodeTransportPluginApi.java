package network.reticulum.emergency;

import android.util.Log;

import com.getcapacitor.JSObject;
import com.getcapacitor.Logger;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.PermissionCallback;

public abstract class ReticulumNodeTransportPluginApi extends ReticulumNodePluginBase {
    @PluginMethod
    public void startNode(PluginCall call) {
        final JSObject config = call.getObject("config", new JSObject());
        Logger.info(TAG, "startNode called.");
        executeBridgeTask(() -> {
            try {
                startServiceForRuntime();
                final ReticulumNodeService service = awaitService();
                final int result = service.startNode(config.toString());
                if (result != 0) {
                    rejectFromNative(call, "Failed to start native Reticulum node.");
                    return;
                }
                call.resolve();
            } catch (Exception ex) {
                call.reject("Failed to start native Reticulum node.", ex);
            }
        });
    }

    @PluginMethod
    public void stopNode(PluginCall call) {
        Logger.info(TAG, "stopNode called.");
        runIntServiceCall(call, "Failed to stop native Reticulum node.", ReticulumNodeService::stopNode);
    }

    @PluginMethod
    public void restartNode(PluginCall call) {
        final JSObject config = call.getObject("config", new JSObject());
        Logger.info(TAG, "restartNode called.");
        executeBridgeTask(() -> {
            try {
                startServiceForRuntime();
                final ReticulumNodeService service = awaitService();
                final int result = service.restartNode(config.toString());
                if (result != 0) {
                    rejectFromNative(call, "Failed to restart native Reticulum node.");
                    return;
                }
                call.resolve();
            } catch (Exception ex) {
                call.reject("Failed to restart native Reticulum node.", ex);
            }
        });
    }

    @PluginMethod
    public void getStatus(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to fetch node status.",
            "Native status JSON parse failed.",
            ReticulumNodeService::getStatusJson
        );
    }

    @PluginMethod
    public void checkRnodeBluetoothPermissions(PluginCall call) {
        resolveRnodeBluetoothPermission(call);
    }

    @PluginMethod
    public void requestRnodeBluetoothPermissions(PluginCall call) {
        if (rnodeBluetoothController().hasPermission()) {
            resolveRnodeBluetoothPermission(call);
            return;
        }
        requestPermissionForAlias(RNODE_BLUETOOTH_ALIAS, call, "completeRnodeBluetoothPermissionRequest");
    }

    @PermissionCallback
    private void completeRnodeBluetoothPermissionRequest(PluginCall call) {
        resolveRnodeBluetoothPermission(call);
    }

    @PluginMethod
    public void listPairedRnodeBluetoothDevices(PluginCall call) {
        rnodeBluetoothController().listPairedDevices(call);
    }

    @PluginMethod
    public void scanRnodeBleDevices(PluginCall call) {
        rnodeBluetoothController().scanDevices(call);
    }

    @PluginMethod
    public void pairRnodeBleDevice(PluginCall call) {
        rnodeBluetoothController().pairDevice(call);
    }

    @PluginMethod
    public void listRnodeUsbDevices(PluginCall call) {
        rnodeUsbPairingController().listDevices(call);
    }

    @PluginMethod
    public void requestRnodeUsbPermission(PluginCall call) {
        rnodeUsbPairingController().requestPermission(call);
    }

    @PluginMethod
    public void startRnodeUsbBluetoothPairing(PluginCall call) {
        rnodeUsbPairingController().startBluetoothPairing(call);
    }

    @PluginMethod
    public void cancelRnodeUsbBluetoothPairing(PluginCall call) {
        rnodeUsbPairingController().cancelBluetoothPairing(call);
    }

    @PluginMethod
    public void connectPeer(PluginCall call) {
        final String destinationHex = call.getString("destinationHex");
        if (destinationHex == null || destinationHex.isEmpty()) {
            call.reject("destinationHex is required.");
            return;
        }
        runIntServiceCall(
            call,
            "Failed to connect peer.",
            service -> service.connectPeer(destinationHex)
        );
    }

    @PluginMethod
    public void disconnectPeer(PluginCall call) {
        final String destinationHex = call.getString("destinationHex");
        if (destinationHex == null || destinationHex.isEmpty()) {
            call.reject("destinationHex is required.");
            return;
        }
        runIntServiceCall(
            call,
            "Failed to disconnect peer.",
            service -> service.disconnectPeer(destinationHex)
        );
    }

    @PluginMethod
    public void send(PluginCall call) {
        final String destinationHex = call.getString("destinationHex");
        final String bytesBase64 = call.getString("bytesBase64");
        final String fieldsBase64 = call.getString("fieldsBase64");
        final String sendMode = call.getString("sendMode");
        final boolean usePropagationNode = call.getBoolean("usePropagationNode", false);
        if (destinationHex == null || destinationHex.isEmpty()) {
            call.reject("destinationHex is required.");
            return;
        }
        if (bytesBase64 == null) {
            call.reject("bytesBase64 is required.");
            return;
        }

        final JSObject payload = new JSObject();
        payload.put("destinationHex", destinationHex);
        payload.put("bytesBase64", bytesBase64);
        if (fieldsBase64 != null && !fieldsBase64.isEmpty()) {
            payload.put("fieldsBase64", fieldsBase64);
        }
        if (sendMode != null && !sendMode.isEmpty()) {
            payload.put("sendMode", sendMode);
        }
        if (usePropagationNode) {
            payload.put("usePropagationNode", true);
        }

        Log.d(
            TAG,
            "send destination="
                + destinationHex
                + " bytesBase64Length="
                + bytesBase64.length()
                + " fieldsBase64Present="
                + (fieldsBase64 != null && !fieldsBase64.isEmpty())
                + " sendMode="
                + (sendMode != null ? sendMode : "Auto")
                + " usePropagationNode="
                + usePropagationNode
        );

        runIntServiceCall(
            call,
            "Failed to send bytes.",
            service -> service.sendJson(payload.toString()),
            () -> Log.d(TAG, "send native accepted destination=" + destinationHex)
        );
    }

    @PluginMethod
    public void sendLxmf(PluginCall call) {
        final String destinationHex = call.getString("destinationHex");
        final String bodyUtf8 = call.getString("bodyUtf8", "");
        final String title = call.getString("title");
        final String sendMode = call.getString("sendMode");
        final boolean usePropagationNode = call.getBoolean("usePropagationNode", false);
        if (destinationHex == null || destinationHex.isEmpty()) {
            call.reject("destinationHex is required.");
            return;
        }

        final JSObject payload = new JSObject();
        payload.put("destinationHex", destinationHex);
        payload.put("bodyUtf8", bodyUtf8);
        if (title != null && !title.isEmpty()) {
            payload.put("title", title);
        }
        if (sendMode != null && !sendMode.isEmpty()) {
            payload.put("sendMode", sendMode);
        }
        if (usePropagationNode) {
            payload.put("usePropagationNode", true);
        }

        runStringServiceCall(
            call,
            "Failed to send LXMF message.",
            "Native LXMF send JSON parse failed.",
            service -> service.sendLxmfJson(payload.toString())
        );
    }

    @PluginMethod
    public void retryLxmf(PluginCall call) {
        final String messageIdHex = call.getString("messageIdHex");
        if (messageIdHex == null || messageIdHex.isEmpty()) {
            call.reject("messageIdHex is required.");
            return;
        }
        final JSObject payload = new JSObject();
        payload.put("messageIdHex", messageIdHex);
        runIntServiceCall(
            call,
            "Failed to retry LXMF message.",
            service -> service.retryLxmfJson(payload.toString())
        );
    }

    @PluginMethod
    public void cancelLxmf(PluginCall call) {
        final String messageIdHex = call.getString("messageIdHex");
        if (messageIdHex == null || messageIdHex.isEmpty()) {
            call.reject("messageIdHex is required.");
            return;
        }
        final JSObject payload = new JSObject();
        payload.put("messageIdHex", messageIdHex);
        runIntServiceCall(
            call,
            "Failed to cancel LXMF message.",
            service -> service.cancelLxmfJson(payload.toString())
        );
    }

    @PluginMethod
    public void announceNow(PluginCall call) {
        runIntServiceCall(call, "Failed to send announce.", ReticulumNodeService::announceNow);
    }

    @PluginMethod
    public void requestPeerIdentity(PluginCall call) {
        final String destinationHex = call.getString("destinationHex");
        if (destinationHex == null || destinationHex.isEmpty()) {
            call.reject("destinationHex is required.");
            return;
        }
        runIntServiceCall(
            call,
            "Failed to request peer identity.",
            service -> service.requestPeerIdentity(destinationHex)
        );
    }

    @PluginMethod
    public void broadcast(PluginCall call) {
        final String bytesBase64 = call.getString("bytesBase64");
        final String fieldsBase64 = call.getString("fieldsBase64");
        if (bytesBase64 == null) {
            call.reject("bytesBase64 is required.");
            return;
        }
        if (fieldsBase64 != null && !fieldsBase64.isEmpty()) {
            call.reject("fieldsBase64 is not supported for broadcast.");
            return;
        }
        runIntServiceCall(
            call,
            "Failed to broadcast bytes.",
            service -> service.broadcastBase64(bytesBase64)
        );
    }

    @PluginMethod
    public void setAnnounceCapabilities(PluginCall call) {
        final String capabilityString = call.getString("capabilityString");
        if (capabilityString == null) {
            call.reject("capabilityString is required.");
            return;
        }
        runIntServiceCall(
            call,
            "Failed to set announce capabilities.",
            service -> service.setAnnounceCapabilities(capabilityString)
        );
    }

    @PluginMethod
    public void setLogLevel(PluginCall call) {
        final String level = call.getString("level", "Info");
        runIntServiceCall(
            call,
            "Failed to set log level.",
            service -> service.setLogLevel(level)
        );
    }

    @PluginMethod
    public void logMessage(PluginCall call) {
        final String level = call.getString("level", "Info");
        final String message = call.getString("message", "");
        writeLogcat(level, "[ui][" + level + "] " + message);
        call.resolve();
    }

    @PluginMethod
    public void refreshHubDirectory(PluginCall call) {
        Logger.info(TAG, "refreshHubDirectory called.");
        runIntServiceCall(call, "Failed to refresh hub directory.", ReticulumNodeService::refreshHubDirectory);
    }

    @PluginMethod
    public void getHubDirectorySnapshot(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to read hub team directory.",
            "Native hub team directory JSON parse failed.",
            ReticulumNodeService::getHubDirectorySnapshotJson
        );
    }

    @PluginMethod
    public void setActiveTeam(PluginCall call) {
        final String teamUid = call.getString("teamUid");
        if (teamUid == null || teamUid.isEmpty()) {
            call.reject("teamUid is required.");
            return;
        }
        final JSObject payload = new JSObject();
        payload.put("teamUid", teamUid);
        runIntServiceCall(
            call,
            "Failed to select active team.",
            service -> service.setActiveTeamJson(payload.toString())
        );
    }
}
