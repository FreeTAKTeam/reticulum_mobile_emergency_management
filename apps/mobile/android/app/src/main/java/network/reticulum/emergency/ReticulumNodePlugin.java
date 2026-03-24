package network.reticulum.emergency;

import android.util.Log;

import com.getcapacitor.JSObject;
import com.getcapacitor.Logger;
import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.CapacitorPlugin;

import java.io.File;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicBoolean;

import org.json.JSONException;

@CapacitorPlugin(name = "ReticulumNode")
public class ReticulumNodePlugin extends Plugin {
    private static final String TAG = "ReticulumNode";

    private final AtomicBoolean pollerRunning = new AtomicBoolean(false);
    private final ExecutorService bridgeExecutor = Executors.newFixedThreadPool(4);
    private ExecutorService poller;

    @Override
    public void load() {
        super.load();
        Logger.info(TAG, "ReticulumNode plugin loaded.");
    }

    @Override
    protected void handleOnDestroy() {
        stopPoller();
        bridgeExecutor.shutdownNow();
        ReticulumBridge.stop();
        super.handleOnDestroy();
    }

    @PluginMethod
    public void startNode(PluginCall call) {
        JSObject config = call.getObject("config", new JSObject());
        normalizeConfig(config);
        Logger.info(TAG, "startNode called.");
        runIntBridgeCall(
            call,
            "Failed to start native Reticulum node.",
            () -> ReticulumBridge.start(config.toString()),
            true
        );
    }

    @PluginMethod
    public void stopNode(PluginCall call) {
        Logger.info(TAG, "stopNode called.");
        runIntBridgeCall(
            call,
            "Failed to stop native Reticulum node.",
            ReticulumBridge::stop,
            false
        );
    }

    @PluginMethod
    public void restartNode(PluginCall call) {
        JSObject config = call.getObject("config", new JSObject());
        normalizeConfig(config);
        Logger.info(TAG, "restartNode called.");
        runIntBridgeCall(
            call,
            "Failed to restart native Reticulum node.",
            () -> ReticulumBridge.restart(config.toString()),
            true
        );
    }

    @PluginMethod
    public void getStatus(PluginCall call) {
        runStringBridgeCall(
            call,
            "Failed to fetch node status.",
            "Native status JSON parse failed.",
            ReticulumBridge::getStatusJson
        );
    }

    @PluginMethod
    public void connectPeer(PluginCall call) {
        String destinationHex = call.getString("destinationHex");
        if (destinationHex == null || destinationHex.isEmpty()) {
            call.reject("destinationHex is required.");
            return;
        }

        runIntBridgeCall(
            call,
            "Failed to connect peer.",
            () -> ReticulumBridge.connectPeer(destinationHex),
            false
        );
    }

    @PluginMethod
    public void disconnectPeer(PluginCall call) {
        String destinationHex = call.getString("destinationHex");
        if (destinationHex == null || destinationHex.isEmpty()) {
            call.reject("destinationHex is required.");
            return;
        }

        runIntBridgeCall(
            call,
            "Failed to disconnect peer.",
            () -> ReticulumBridge.disconnectPeer(destinationHex),
            false
        );
    }

    @PluginMethod
    public void send(PluginCall call) {
        String destinationHex = call.getString("destinationHex");
        String bytesBase64 = call.getString("bytesBase64");
        String fieldsBase64 = call.getString("fieldsBase64");
        String sendMode = call.getString("sendMode");
        boolean usePropagationNode = call.getBoolean("usePropagationNode", false);
        if (destinationHex == null || destinationHex.isEmpty()) {
            call.reject("destinationHex is required.");
            return;
        }
        if (bytesBase64 == null) {
            call.reject("bytesBase64 is required.");
            return;
        }

        JSObject payload = new JSObject();
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

        runIntBridgeCall(
            call,
            "Failed to send bytes.",
            () -> ReticulumBridge.sendJson(payload.toString()),
            false,
            () -> Log.d(TAG, "send native accepted destination=" + destinationHex)
        );
    }

    @PluginMethod
    public void sendLxmf(PluginCall call) {
        String destinationHex = call.getString("destinationHex");
        String bodyUtf8 = call.getString("bodyUtf8", "");
        String title = call.getString("title");
        String sendMode = call.getString("sendMode");
        boolean usePropagationNode = call.getBoolean("usePropagationNode", false);
        if (destinationHex == null || destinationHex.isEmpty()) {
            call.reject("destinationHex is required.");
            return;
        }

        JSObject payload = new JSObject();
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

        runStringBridgeCall(
            call,
            "Failed to send LXMF message.",
            "Native LXMF send JSON parse failed.",
            () -> ReticulumBridge.sendLxmfJson(payload.toString())
        );
    }

    @PluginMethod
    public void retryLxmf(PluginCall call) {
        String messageIdHex = call.getString("messageIdHex");
        if (messageIdHex == null || messageIdHex.isEmpty()) {
            call.reject("messageIdHex is required.");
            return;
        }

        JSObject payload = new JSObject();
        payload.put("messageIdHex", messageIdHex);
        runIntBridgeCall(
            call,
            "Failed to retry LXMF message.",
            () -> ReticulumBridge.retryLxmfJson(payload.toString()),
            false
        );
    }

    @PluginMethod
    public void cancelLxmf(PluginCall call) {
        String messageIdHex = call.getString("messageIdHex");
        if (messageIdHex == null || messageIdHex.isEmpty()) {
            call.reject("messageIdHex is required.");
            return;
        }

        JSObject payload = new JSObject();
        payload.put("messageIdHex", messageIdHex);
        runIntBridgeCall(
            call,
            "Failed to cancel LXMF message.",
            () -> ReticulumBridge.cancelLxmfJson(payload.toString()),
            false
        );
    }

    @PluginMethod
    public void announceNow(PluginCall call) {
        runIntBridgeCall(
            call,
            "Failed to send announce.",
            ReticulumBridge::announceNow,
            false
        );
    }

    @PluginMethod
    public void requestPeerIdentity(PluginCall call) {
        String destinationHex = call.getString("destinationHex");
        if (destinationHex == null || destinationHex.isEmpty()) {
            call.reject("destinationHex is required.");
            return;
        }

        runIntBridgeCall(
            call,
            "Failed to request peer identity.",
            () -> ReticulumBridge.requestPeerIdentity(destinationHex),
            false
        );
    }

    @PluginMethod
    public void broadcast(PluginCall call) {
        String bytesBase64 = call.getString("bytesBase64");
        String fieldsBase64 = call.getString("fieldsBase64");
        if (bytesBase64 == null) {
            call.reject("bytesBase64 is required.");
            return;
        }
        if (fieldsBase64 != null && !fieldsBase64.isEmpty()) {
            call.reject("fieldsBase64 is not supported for broadcast.");
            return;
        }

        runIntBridgeCall(
            call,
            "Failed to broadcast bytes.",
            () -> ReticulumBridge.broadcastBase64(bytesBase64),
            false
        );
    }

    @PluginMethod
    public void setAnnounceCapabilities(PluginCall call) {
        String capabilityString = call.getString("capabilityString");
        if (capabilityString == null) {
            call.reject("capabilityString is required.");
            return;
        }

        runIntBridgeCall(
            call,
            "Failed to set announce capabilities.",
            () -> ReticulumBridge.setAnnounceCapabilities(capabilityString),
            false
        );
    }

    @PluginMethod
    public void setLogLevel(PluginCall call) {
        String level = call.getString("level", "Info");
        runIntBridgeCall(
            call,
            "Failed to set log level.",
            () -> ReticulumBridge.setLogLevel(level),
            false
        );
    }

    @PluginMethod
    public void logMessage(PluginCall call) {
        String level = call.getString("level", "Info");
        String message = call.getString("message", "");
        writeLogcat(level, "[ui][" + level + "] " + message);
        call.resolve();
    }

    @PluginMethod
    public void refreshHubDirectory(PluginCall call) {
        Logger.info(TAG, "refreshHubDirectory called.");
        runIntBridgeCall(
            call,
            "Failed to refresh hub directory.",
            ReticulumBridge::refreshHubDirectory,
            false
        );
    }

    @PluginMethod
    public void setActivePropagationNode(PluginCall call) {
        JSObject payload = new JSObject();
        payload.put("destinationHex", call.getString("destinationHex"));
        runIntBridgeCall(
            call,
            "Failed to set active propagation node.",
            () -> ReticulumBridge.setActivePropagationNodeJson(payload.toString()),
            false
        );
    }

    @PluginMethod
    public void requestLxmfSync(PluginCall call) {
        JSObject payload = new JSObject();
        Integer limit = call.getInt("limit");
        if (limit != null) {
            payload.put("limit", limit);
        } else {
            payload.put("limit", null);
        }
        runIntBridgeCall(
            call,
            "Failed to request LXMF sync.",
            () -> ReticulumBridge.requestLxmfSyncJson(payload.toString()),
            false
        );
    }

    @PluginMethod
    public void listAnnounces(PluginCall call) {
        runStringBridgeCall(
            call,
            "Failed to list announces.",
            "Native announce list JSON parse failed.",
            ReticulumBridge::listAnnouncesJson
        );
    }

    @PluginMethod
    public void listPeers(PluginCall call) {
        runStringBridgeCall(
            call,
            "Failed to list peers.",
            "Native peer list JSON parse failed.",
            ReticulumBridge::listPeersJson
        );
    }

    @PluginMethod
    public void listConversations(PluginCall call) {
        runStringBridgeCall(
            call,
            "Failed to list conversations.",
            "Native conversation list JSON parse failed.",
            ReticulumBridge::listConversationsJson
        );
    }

    @PluginMethod
    public void listMessages(PluginCall call) {
        JSObject payload = new JSObject();
        payload.put("conversationId", call.getString("conversationId"));
        runStringBridgeCall(
            call,
            "Failed to list messages.",
            "Native message list JSON parse failed.",
            () -> ReticulumBridge.listMessagesJson(payload.toString())
        );
    }

    @PluginMethod
    public void getLxmfSyncStatus(PluginCall call) {
        runStringBridgeCall(
            call,
            "Failed to get LXMF sync status.",
            "Native sync status JSON parse failed.",
            ReticulumBridge::getLxmfSyncStatusJson
        );
    }

    @PluginMethod
    public void removeAllListeners(PluginCall call) {
        stopPoller();
        call.resolve();
    }

    private void ensurePoller() {
        if (!pollerRunning.compareAndSet(false, true)) {
            return;
        }

        poller = Executors.newSingleThreadExecutor();
        poller.execute(() -> {
            while (pollerRunning.get()) {
                try {
                    String raw = ReticulumBridge.nextEventJson(500);
                    if (raw == null || raw.isEmpty()) {
                        continue;
                    }

                    JSObject envelope = new JSObject(raw);
                    String eventName = envelope.getString("event");
                    JSObject payload = envelope.getJSObject("payload", new JSObject());
                    if (eventName != null && !eventName.isEmpty()) {
                        mirrorEventToLogcat(eventName, payload);
                        notifyListeners(eventName, payload);
                    }
                } catch (Exception ex) {
                    Logger.error(TAG, "Event poll loop error", ex);
                }
            }
        });
    }

    private void mirrorEventToLogcat(String eventName, JSObject payload) {
        if ("log".equals(eventName)) {
            String level = payload.getString("level", "Info");
            String message = payload.getString("message", payload.toString());
            writeLogcat(level, message);
            return;
        }

        if (
            "lxmfDelivery".equals(eventName)
                || "packetReceived".equals(eventName)
                || "packetSent".equals(eventName)
                || "announceReceived".equals(eventName)
        ) {
            Log.i(TAG, "[" + eventName + "] " + abbreviate(payload.toString()));
        }
    }

    private void writeLogcat(String level, String message) {
        int priority;
        switch (level) {
            case "Trace":
            case "Debug":
                priority = Log.DEBUG;
                break;
            case "Warn":
                priority = Log.WARN;
                break;
            case "Error":
                priority = Log.ERROR;
                break;
            case "Info":
            default:
                priority = Log.INFO;
                break;
        }

        Log.println(priority, TAG, abbreviate(message));
    }

    private String abbreviate(String value) {
        if (value == null) {
            return "";
        }
        final int maxLength = 4000;
        if (value.length() <= maxLength) {
            return value;
        }
        return value.substring(0, maxLength) + "…";
    }

    private void stopPoller() {
        pollerRunning.set(false);
        if (poller != null) {
            poller.shutdownNow();
            poller = null;
        }
    }

    private void runIntBridgeCall(
        PluginCall call,
        String fallbackMessage,
        NativeIntOperation operation,
        boolean onSuccessEnsurePoller
    ) {
        runIntBridgeCall(call, fallbackMessage, operation, onSuccessEnsurePoller, null);
    }

    private void runIntBridgeCall(
        PluginCall call,
        String fallbackMessage,
        NativeIntOperation operation,
        boolean onSuccessEnsurePoller,
        Runnable onSuccess
    ) {
        bridgeExecutor.execute(
            () -> {
                try {
                    int result = operation.run();
                    if (result != 0) {
                        rejectFromNative(call, fallbackMessage);
                        return;
                    }
                    if (onSuccessEnsurePoller) {
                        ensurePoller();
                    }
                    if (onSuccess != null) {
                        onSuccess.run();
                    }
                    call.resolve();
                } catch (Exception ex) {
                    call.reject(fallbackMessage, ex);
                }
            }
        );
    }

    private void runStringBridgeCall(
        PluginCall call,
        String fallbackMessage,
        String parseFallbackMessage,
        NativeStringOperation operation
    ) {
        bridgeExecutor.execute(
            () -> {
                try {
                    String raw = operation.run();
                    if (raw == null || raw.isEmpty()) {
                        rejectFromNative(call, fallbackMessage);
                        return;
                    }
                    resolveJson(call, raw, parseFallbackMessage);
                } catch (Exception ex) {
                    call.reject(fallbackMessage, ex);
                }
            }
        );
    }

    private interface NativeIntOperation {
        int run() throws Exception;
    }

    private interface NativeStringOperation {
        String run() throws Exception;
    }

    private void rejectFromNative(PluginCall call, String fallbackMessage) {
        String raw = ReticulumBridge.takeLastErrorJson();
        if (raw == null || raw.isEmpty()) {
            Logger.error(TAG, fallbackMessage, new Exception(fallbackMessage));
            call.reject(fallbackMessage);
            return;
        }

        try {
            JSObject payload = new JSObject(raw);
            String code = payload.getString("code", "NativeError");
            String message = payload.getString("message", fallbackMessage);
            Log.e(TAG, "rejectFromNative code=" + code + " message=" + message);
            Logger.error(TAG, "Native error [" + code + "]: " + message, new Exception(message));
            call.reject(message, code);
        } catch (JSONException ex) {
            call.reject(fallbackMessage, ex);
        }
    }

    private void resolveJson(PluginCall call, String raw, String fallbackMessage) {
        try {
            call.resolve(new JSObject(raw));
        } catch (JSONException ex) {
            call.reject(fallbackMessage, ex);
        }
    }

    private void normalizeConfig(JSObject config) {
        String rawStorageDir = config.getString("storageDir", "");
        String storageDir = rawStorageDir == null ? "" : rawStorageDir.trim();

        File filesDir = getContext().getFilesDir();
        File resolved;
        if (storageDir.isEmpty()) {
            resolved = new File(filesDir, "reticulum-mobile");
        } else {
            File candidate = new File(storageDir);
            resolved = candidate.isAbsolute() ? candidate : new File(filesDir, storageDir);
        }

        config.put("storageDir", resolved.getAbsolutePath());
    }
}
