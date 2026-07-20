package network.reticulum.emergency;

import android.os.Handler;
import android.util.Log;

import com.getcapacitor.JSObject;
import com.getcapacitor.Logger;

import network.reticulum.emergency.plugins.PluginCoordinator;

import org.json.JSONException;
import org.json.JSONObject;

import java.util.concurrent.CopyOnWriteArraySet;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicBoolean;

final class ServiceEventCoordinator {
    interface ForegroundState {
        boolean isForeground();
    }

    interface NotificationUpdater {
        void update();
    }

    private static final String TAG = "ReticulumNodeService";

    private final Handler mainHandler;
    private final ServiceNotificationController notificationController;
    private final PluginCoordinator pluginCoordinator;
    private final SosPlatformCoordinator sosPlatformCoordinator;
    private final ForegroundState foregroundState;
    private final NotificationUpdater notificationUpdater;
    private final CopyOnWriteArraySet<ReticulumNodeService.ServiceEventListener> listeners =
        new CopyOnWriteArraySet<>();
    private final AtomicBoolean pollerRunning = new AtomicBoolean(false);
    private final ExecutorService pollerExecutor = Executors.newSingleThreadExecutor();

    private volatile String latestStatusJson = "";
    private volatile String latestSyncStatusJson = "";
    private volatile String latestSosStatusJson = "";
    private volatile String latestRuntimeErrorJson = "";

    ServiceEventCoordinator(
        Handler mainHandler,
        ServiceNotificationController notificationController,
        PluginCoordinator pluginCoordinator,
        SosPlatformCoordinator sosPlatformCoordinator,
        ForegroundState foregroundState,
        NotificationUpdater notificationUpdater
    ) {
        this.mainHandler = mainHandler;
        this.notificationController = notificationController;
        this.pluginCoordinator = pluginCoordinator;
        this.sosPlatformCoordinator = sosPlatformCoordinator;
        this.foregroundState = foregroundState;
        this.notificationUpdater = notificationUpdater;
    }

    void start() {
        if (!pollerRunning.compareAndSet(false, true)) {
            return;
        }

        pollerExecutor.execute(() -> {
            while (pollerRunning.get()) {
                try {
                    final String raw = ReticulumBridge.nextEventJson(500);
                    if (raw == null || raw.isEmpty()) {
                        continue;
                    }

                    final JSONObject envelope = new JSONObject(raw);
                    final String eventName = envelope.optString("event", "");
                    JSONObject payload = envelope.optJSONObject("payload");
                    if (payload == null) {
                        payload = new JSONObject();
                    }
                    handleNativeEvent(eventName, new JSObject(payload.toString()));
                } catch (Exception ex) {
                    Logger.error(TAG, "Service event poll loop error", ex);
                }
            }
        });
    }

    void stop() {
        pollerRunning.set(false);
    }

    void close() {
        stop();
        pollerExecutor.shutdownNow();
    }

    void addListener(ReticulumNodeService.ServiceEventListener listener) {
        if (listener == null) {
            return;
        }
        listeners.add(listener);
        mainHandler.post(() -> {
            emitCachedState(listener);
            emitProjectionRefreshSweep(listener);
        });
    }

    void removeListener(ReticulumNodeService.ServiceEventListener listener) {
        if (listener != null) {
            listeners.remove(listener);
        }
    }

    void clearRuntimeReadinessFailure() {
        latestRuntimeErrorJson = "";
    }

    void reportRuntimeReadinessFailure(String code, String message) {
        final String safeCode = code == null || code.trim().isEmpty() ? "InternalError" : code;
        final String safeMessage =
            message == null || message.trim().isEmpty() ? "node runtime failed" : message;
        Log.e(TAG, safeMessage);
        final JSObject errorPayload = new JSObject();
        errorPayload.put("code", safeCode);
        errorPayload.put("message", safeMessage);
        latestRuntimeErrorJson = errorPayload.toString();
        latestStatusJson = statusJsonWithLastError(safeMessage);
        dispatchEventToListeners("error", errorPayload);
        final JSObject statusPayload = new JSObject();
        try {
            statusPayload.put(
                "status",
                new JSObject(JsonPayloads.orFallback(latestStatusJson, "{}"))
            );
        } catch (JSONException error) {
            Log.w(TAG, "Unable to decode the cached runtime status event", error);
            statusPayload.put("status", new JSObject());
        }
        dispatchEventToListeners("statusChanged", statusPayload);
        notificationUpdater.update();
    }

    void emitCachedStateToAll() {
        for (ReticulumNodeService.ServiceEventListener listener : listeners) {
            emitCachedState(listener);
        }
    }

    void emitProjectionRefreshSweepToAll() {
        for (ReticulumNodeService.ServiceEventListener listener : listeners) {
            emitProjectionRefreshSweep(listener);
        }
    }

    void refreshLatestRuntimeState() {
        latestStatusJson = JsonPayloads.orFallback(ReticulumBridge.getStatusJson(), "{}");
        latestSyncStatusJson = JsonPayloads.orFallback(ReticulumBridge.getLxmfSyncStatusJson(), "{}");
        latestSosStatusJson = JsonPayloads.orFallback(ReticulumBridge.getSosStatusJson(), "{}");
        applyCurrentSosPlatformSettings();
    }

    void applyCurrentSosPlatformSettings() {
        if (sosPlatformCoordinator != null) {
            sosPlatformCoordinator.applySettingsJson(
                JsonPayloads.orFallback(ReticulumBridge.getSosSettingsJson(), "{}")
            );
        }
    }

    String latestStatusJson() {
        return latestStatusJson;
    }

    String latestSyncStatusJson() {
        return latestSyncStatusJson;
    }

    private void handleNativeEvent(String eventName, JSObject payload) {
        if (eventName == null || eventName.isEmpty()) {
            return;
        }
        mirrorEventToLogcat(eventName, payload);
        updateCachedState(eventName, payload);
        if ("packetReceived".equals(eventName) && pluginCoordinator != null) {
            final String fieldsBase64 = payload.getString("fieldsBase64", "");
            if (!fieldsBase64.isEmpty()) {
                final String decoded = ReticulumBridge.decodePluginLxmfFieldsJson(fieldsBase64);
                if (decoded != null && !decoded.isEmpty() && !"null".equals(decoded)) {
                    try {
                        pluginCoordinator.dispatchPluginLxmf(new JSONObject(decoded));
                    } catch (JSONException error) {
                        Logger.error(TAG, "Invalid plugin LXMF envelope", error);
                    }
                }
            }
        }
        dispatchEventToListeners(eventName, payload);
        final boolean uiForeground = foregroundState.isForeground();
        if ("sosAlertChanged".equals(eventName)) {
            notificationController.handleSosAlert(payload, !uiForeground);
        } else if (!uiForeground) {
            notificationController.handleInboundUpdate(eventName, payload);
        }
        if ("sosTelemetryRequested".equals(eventName) && sosPlatformCoordinator != null) {
            sosPlatformCoordinator.submitTelemetrySnapshot();
        }
        if ("statusChanged".equals(eventName) || "syncUpdated".equals(eventName)) {
            notificationUpdater.update();
        }
    }

    private void mirrorEventToLogcat(String eventName, JSObject payload) {
        if ("log".equals(eventName)) {
            final String level = payload.getString("level", "Info");
            writeLogcat(level, payload.getString("message", payload.toString()));
            return;
        }
        if (
            "lxmfDelivery".equals(eventName)
                || "packetReceived".equals(eventName)
                || "packetSent".equals(eventName)
                || "announceReceived".equals(eventName)
                || "messageReceived".equals(eventName)
                || "sosAlertChanged".equals(eventName)
        ) {
            Log.i(TAG, "[" + eventName + "] " + abbreviate(payload.toString()));
        }
    }

    private void updateCachedState(String eventName, JSObject payload) {
        if ("statusChanged".equals(eventName)) {
            try {
                final JSObject status = payload.getJSObject("status", payload);
                latestStatusJson = status.toString();
            } catch (JSONException error) {
                Log.d(TAG, "Status event uses the legacy unwrapped payload shape", error);
                latestStatusJson = payload.toString();
            }
            return;
        }
        if ("syncUpdated".equals(eventName)) {
            latestSyncStatusJson = payload.toString();
            return;
        }
        if ("sosStatusChanged".equals(eventName)) {
            try {
                final JSObject status = payload.getJSObject("status", payload);
                latestSosStatusJson = status.toString();
            } catch (JSONException error) {
                Log.d(TAG, "SOS event uses the legacy unwrapped payload shape", error);
                latestSosStatusJson = payload.toString();
            }
        }
    }

    private void dispatchEventToListeners(String eventName, JSObject payload) {
        if (!NativeEventBackpressure.shouldDispatchToUi(eventName, payload)) {
            return;
        }
        for (ReticulumNodeService.ServiceEventListener listener : listeners) {
            mainHandler.post(() -> listener.onNodeEvent(eventName, payload));
        }
    }

    private void emitCachedState(ReticulumNodeService.ServiceEventListener listener) {
        if (listener == null) {
            return;
        }
        try {
            final JSObject statusPayload = new JSObject();
            statusPayload.put(
                "status",
                new JSObject(JsonPayloads.orFallback(latestStatusJson, "{}"))
            );
            listener.onNodeEvent("statusChanged", statusPayload);
        } catch (JSONException error) {
            Log.w(TAG, "Unable to replay the cached runtime status event", error);
            listener.onNodeEvent("statusChanged", new JSObject());
        }

        try {
            listener.onNodeEvent(
                "syncUpdated",
                new JSObject(JsonPayloads.orFallback(latestSyncStatusJson, "{}"))
            );
        } catch (JSONException error) {
            Log.w(TAG, "Unable to replay the cached sync event", error);
            listener.onNodeEvent("syncUpdated", new JSObject());
        }

        try {
            final JSObject statusPayload = new JSObject();
            statusPayload.put(
                "status",
                new JSObject(JsonPayloads.orFallback(latestSosStatusJson, "{}"))
            );
            listener.onNodeEvent("sosStatusChanged", statusPayload);
        } catch (JSONException error) {
            Log.w(TAG, "Unable to replay the cached SOS status event", error);
            listener.onNodeEvent("sosStatusChanged", new JSObject());
        }

        if (latestRuntimeErrorJson != null && !latestRuntimeErrorJson.trim().isEmpty()) {
            try {
                listener.onNodeEvent("error", new JSObject(latestRuntimeErrorJson));
            } catch (JSONException error) {
                Log.w(TAG, "Unable to replay the structured runtime error", error);
                final JSObject fallback = new JSObject();
                fallback.put("code", "InternalError");
                fallback.put("message", "node runtime failed");
                listener.onNodeEvent("error", fallback);
            }
        }
    }

    private void emitProjectionRefreshSweep(ReticulumNodeService.ServiceEventListener listener) {
        if (listener == null) {
            return;
        }
        for (String scope : new String[] {
            "AppSettings",
            "SavedPeers",
            "OperationalSummary",
            "Peers",
            "SyncStatus",
            "HubRegistration",
            "Checklists",
            "ChecklistDetail",
            "Eams",
            "Events",
            "Conversations",
            "Messages",
            "Telemetry",
            "Sos",
            "Plugins",
            "PluginSensors",
        }) {
            final JSObject payload = new JSObject();
            payload.put("scope", scope);
            payload.put("revision", 0);
            payload.put("updatedAtMs", System.currentTimeMillis());
            payload.put("reason", "serviceRebind");
            listener.onNodeEvent("projectionInvalidated", payload);
        }
    }

    private String statusJsonWithLastError(String message) {
        try {
            final JSONObject status = new JSONObject(
                JsonPayloads.orFallback(ReticulumBridge.getStatusJson(), "{}")
            );
            status.put("running", false);
            status.put("lastError", message);
            return status.toString();
        } catch (JSONException ex) {
            final JSONObject status = new JSONObject();
            try {
                status.put("running", false);
                status.put("lastError", message);
            } catch (JSONException error) {
                Log.w(TAG, "Unable to augment the runtime failure status", error);
                return "{\"running\":false}";
            }
            return status.toString();
        }
    }

    private void writeLogcat(String level, String message) {
        final int priority;
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
        return value.substring(0, maxLength) + "...";
    }
}
