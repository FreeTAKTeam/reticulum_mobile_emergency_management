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
    private ExecutorService poller;

    @Override
    public void load() {
        super.load();
        Logger.info(TAG, "ReticulumNode plugin loaded.");
        ensurePoller();
    }

    @Override
    protected void handleOnDestroy() {
        stopPoller();
        ReticulumBridge.stop();
        super.handleOnDestroy();
    }

    @PluginMethod
    public void startNode(PluginCall call) {
        JSObject config = call.getObject("config", new JSObject());
        normalizeConfig(config);
        Logger.info(TAG, "startNode called.");
        int result = ReticulumBridge.start(config.toString());
        if (result != 0) {
            rejectFromNative(call, "Failed to start native Reticulum node.");
            return;
        }
        ensurePoller();
        call.resolve();
    }

    @PluginMethod
    public void stopNode(PluginCall call) {
        Logger.info(TAG, "stopNode called.");
        int result = ReticulumBridge.stop();
        if (result != 0) {
            rejectFromNative(call, "Failed to stop native Reticulum node.");
            return;
        }
        call.resolve();
    }

    @PluginMethod
    public void restartNode(PluginCall call) {
        JSObject config = call.getObject("config", new JSObject());
        normalizeConfig(config);
        Logger.info(TAG, "restartNode called.");
        int result = ReticulumBridge.restart(config.toString());
        if (result != 0) {
            rejectFromNative(call, "Failed to restart native Reticulum node.");
            return;
        }
        ensurePoller();
        call.resolve();
    }

    @PluginMethod
    public void getStatus(PluginCall call) {
        String raw = ReticulumBridge.getStatusJson();
        if (raw == null || raw.isEmpty()) {
            rejectFromNative(call, "Failed to fetch node status.");
            return;
        }

        try {
            call.resolve(new JSObject(raw));
        } catch (JSONException ex) {
            call.reject("Native status JSON parse failed.", ex);
        }
    }

    @PluginMethod
    public void connectPeer(PluginCall call) {
        String destinationHex = call.getString("destinationHex");
        if (destinationHex == null || destinationHex.isEmpty()) {
            call.reject("destinationHex is required.");
            return;
        }

        int result = ReticulumBridge.connectPeer(destinationHex);
        if (result != 0) {
            rejectFromNative(call, "Failed to connect peer.");
            return;
        }
        call.resolve();
    }

    @PluginMethod
    public void disconnectPeer(PluginCall call) {
        String destinationHex = call.getString("destinationHex");
        if (destinationHex == null || destinationHex.isEmpty()) {
            call.reject("destinationHex is required.");
            return;
        }

        int result = ReticulumBridge.disconnectPeer(destinationHex);
        if (result != 0) {
            rejectFromNative(call, "Failed to disconnect peer.");
            return;
        }
        call.resolve();
    }

    @PluginMethod
    public void send(PluginCall call) {
        String destinationHex = call.getString("destinationHex");
        String bytesBase64 = call.getString("bytesBase64");
        String fieldsBase64 = call.getString("fieldsBase64");
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

        Log.d(
            TAG,
            "send destination="
                + destinationHex
                + " bytesBase64Length="
                + bytesBase64.length()
                + " fieldsBase64Present="
                + (fieldsBase64 != null && !fieldsBase64.isEmpty())
        );

        int result = ReticulumBridge.sendJson(payload.toString());
        if (result != 0) {
            Log.e(TAG, "send native returned non-zero destination=" + destinationHex);
            rejectFromNative(call, "Failed to send bytes.");
            return;
        }
        Log.d(TAG, "send native accepted destination=" + destinationHex);
        call.resolve();
    }

    @PluginMethod
    public void sendLxmf(PluginCall call) {
        String destinationHex = call.getString("destinationHex");
        String bodyUtf8 = call.getString("bodyUtf8", "");
        String title = call.getString("title");
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

        String raw = ReticulumBridge.sendLxmfJson(payload.toString());
        if (raw == null || raw.isEmpty()) {
            rejectFromNative(call, "Failed to send LXMF message.");
            return;
        }
        resolveJson(call, raw, "Native LXMF send JSON parse failed.");
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
        int result = ReticulumBridge.retryLxmfJson(payload.toString());
        if (result != 0) {
            rejectFromNative(call, "Failed to retry LXMF message.");
            return;
        }
        call.resolve();
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
        int result = ReticulumBridge.cancelLxmfJson(payload.toString());
        if (result != 0) {
            rejectFromNative(call, "Failed to cancel LXMF message.");
            return;
        }
        call.resolve();
    }

    @PluginMethod
    public void announceNow(PluginCall call) {
        int result = ReticulumBridge.announceNow();
        if (result != 0) {
            rejectFromNative(call, "Failed to send announce.");
            return;
        }
        call.resolve();
    }

    @PluginMethod
    public void requestPeerIdentity(PluginCall call) {
        String destinationHex = call.getString("destinationHex");
        if (destinationHex == null || destinationHex.isEmpty()) {
            call.reject("destinationHex is required.");
            return;
        }

        int result = ReticulumBridge.requestPeerIdentity(destinationHex);
        if (result != 0) {
            rejectFromNative(call, "Failed to request peer identity.");
            return;
        }
        call.resolve();
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

        int result = ReticulumBridge.broadcastBase64(bytesBase64);
        if (result != 0) {
            rejectFromNative(call, "Failed to broadcast bytes.");
            return;
        }
        call.resolve();
    }

    @PluginMethod
    public void setAnnounceCapabilities(PluginCall call) {
        String capabilityString = call.getString("capabilityString");
        if (capabilityString == null) {
            call.reject("capabilityString is required.");
            return;
        }

        int result = ReticulumBridge.setAnnounceCapabilities(capabilityString);
        if (result != 0) {
            rejectFromNative(call, "Failed to set announce capabilities.");
            return;
        }
        call.resolve();
    }

    @PluginMethod
    public void setLogLevel(PluginCall call) {
        String level = call.getString("level", "Info");
        int result = ReticulumBridge.setLogLevel(level);
        if (result != 0) {
            rejectFromNative(call, "Failed to set log level.");
            return;
        }
        call.resolve();
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
        int result = ReticulumBridge.refreshHubDirectory();
        if (result != 0) {
            rejectFromNative(call, "Failed to refresh hub directory.");
            return;
        }
        call.resolve();
    }

    @PluginMethod
    public void setActivePropagationNode(PluginCall call) {
        JSObject payload = new JSObject();
        payload.put("destinationHex", call.getString("destinationHex"));
        int result = ReticulumBridge.setActivePropagationNodeJson(payload.toString());
        if (result != 0) {
            rejectFromNative(call, "Failed to set active propagation node.");
            return;
        }
        call.resolve();
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
        int result = ReticulumBridge.requestLxmfSyncJson(payload.toString());
        if (result != 0) {
            rejectFromNative(call, "Failed to request LXMF sync.");
            return;
        }
        call.resolve();
    }

    @PluginMethod
    public void listAnnounces(PluginCall call) {
        String raw = ReticulumBridge.listAnnouncesJson();
        if (raw == null || raw.isEmpty()) {
            rejectFromNative(call, "Failed to list announces.");
            return;
        }
        resolveJson(call, raw, "Native announce list JSON parse failed.");
    }

    @PluginMethod
    public void listPeers(PluginCall call) {
        String raw = ReticulumBridge.listPeersJson();
        if (raw == null || raw.isEmpty()) {
            rejectFromNative(call, "Failed to list peers.");
            return;
        }
        resolveJson(call, raw, "Native peer list JSON parse failed.");
    }

    @PluginMethod
    public void listConversations(PluginCall call) {
        String raw = ReticulumBridge.listConversationsJson();
        if (raw == null || raw.isEmpty()) {
            rejectFromNative(call, "Failed to list conversations.");
            return;
        }
        resolveJson(call, raw, "Native conversation list JSON parse failed.");
    }

    @PluginMethod
    public void listMessages(PluginCall call) {
        JSObject payload = new JSObject();
        payload.put("conversationId", call.getString("conversationId"));
        String raw = ReticulumBridge.listMessagesJson(payload.toString());
        if (raw == null || raw.isEmpty()) {
            rejectFromNative(call, "Failed to list messages.");
            return;
        }
        resolveJson(call, raw, "Native message list JSON parse failed.");
    }

    @PluginMethod
    public void getLxmfSyncStatus(PluginCall call) {
        String raw = ReticulumBridge.getLxmfSyncStatusJson();
        if (raw == null || raw.isEmpty()) {
            rejectFromNative(call, "Failed to get LXMF sync status.");
            return;
        }
        resolveJson(call, raw, "Native sync status JSON parse failed.");
    }

    @PluginMethod
    public void removeAllListeners(PluginCall call) {
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
