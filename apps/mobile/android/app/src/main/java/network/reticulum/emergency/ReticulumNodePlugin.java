package network.reticulum.emergency;

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

        int result = ReticulumBridge.sendJson(payload.toString());
        if (result != 0) {
            rejectFromNative(call, "Failed to send bytes.");
            return;
        }
        call.resolve();
    }

    @PluginMethod
    public void broadcast(PluginCall call) {
        String bytesBase64 = call.getString("bytesBase64");
        if (bytesBase64 == null) {
            call.reject("bytesBase64 is required.");
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
                        notifyListeners(eventName, payload);
                    }
                } catch (Exception ex) {
                    Logger.error(TAG, "Event poll loop error", ex);
                }
            }
        });
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
            Logger.error(TAG, "Native error [" + code + "]: " + message, new Exception(message));
            call.reject(message, code);
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
