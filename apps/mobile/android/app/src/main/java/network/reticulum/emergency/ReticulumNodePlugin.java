package network.reticulum.emergency;

import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.ServiceConnection;
import android.os.Build;
import android.os.IBinder;
import android.util.Log;

import androidx.core.content.ContextCompat;

import com.getcapacitor.JSObject;
import com.getcapacitor.Logger;
import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.CapacitorPlugin;

import org.json.JSONException;

import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;

@CapacitorPlugin(name = "ReticulumNode")
public class ReticulumNodePlugin extends Plugin {
    private static final String TAG = "ReticulumNode";
    private static final long SERVICE_BIND_TIMEOUT_MS = 10_000L;

    private final ExecutorService bridgeExecutor = Executors.newFixedThreadPool(4);
    private final ReticulumNodeService.ServiceEventListener serviceEventListener = (eventName, payload) -> {
        final JSObject safePayload = payload == null ? new JSObject() : payload;
        mirrorEventToLogcat(eventName, safePayload);
        notifyListeners(eventName, safePayload);
    };

    private final ServiceConnection serviceConnection = new ServiceConnection() {
        @Override
        public void onServiceConnected(ComponentName name, IBinder service) {
            if (!(service instanceof ReticulumNodeService.LocalBinder)) {
                Logger.error(TAG, "Unexpected binder for ReticulumNodeService", null);
                return;
            }

            final ReticulumNodeService.LocalBinder localBinder = (ReticulumNodeService.LocalBinder) service;
            boundService = localBinder.getService();
            serviceBound = true;
            tryRegisterServiceListener();
            serviceFuture.complete(boundService);
            Logger.info(TAG, "Bound to ReticulumNodeService.");
        }

        @Override
        public void onServiceDisconnected(ComponentName name) {
            unregisterServiceListener();
            boundService = null;
            serviceBound = false;
            resetServiceFuture();
            Logger.info(TAG, "ReticulumNodeService disconnected.");
        }

        @Override
        public void onBindingDied(ComponentName name) {
            unregisterServiceListener();
            boundService = null;
            serviceBound = false;
            resetServiceFuture();
            bindToService();
        }

        @Override
        public void onNullBinding(ComponentName name) {
            unregisterServiceListener();
            boundService = null;
            serviceBound = false;
            resetServiceFuture();
            Logger.error(TAG, "ReticulumNodeService returned null binding.", null);
        }
    };

    private volatile ReticulumNodeService boundService;
    private volatile boolean serviceBound = false;
    private volatile boolean serviceListenerRegistered = false;
    private CompletableFuture<ReticulumNodeService> serviceFuture = new CompletableFuture<>();

    @Override
    public void load() {
        super.load();
        Logger.info(TAG, "ReticulumNode plugin loaded.");
    }

    @Override
    protected void handleOnDestroy() {
        unregisterServiceListener();
        unbindFromService();
        bridgeExecutor.shutdownNow();
        super.handleOnDestroy();
    }

    @PluginMethod
    public void startNode(PluginCall call) {
        final JSObject config = call.getObject("config", new JSObject());
        Logger.info(TAG, "startNode called.");
        bridgeExecutor.execute(() -> {
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
        bridgeExecutor.execute(() -> {
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
    public void setActivePropagationNode(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("destinationHex", call.getString("destinationHex"));
        runIntServiceCall(
            call,
            "Failed to set active propagation node.",
            service -> service.setActivePropagationNodeJson(payload.toString())
        );
    }

    @PluginMethod
    public void requestLxmfSync(PluginCall call) {
        final JSObject payload = new JSObject();
        final Integer limit = call.getInt("limit");
        if (limit != null) {
            payload.put("limit", limit);
        } else {
            payload.put("limit", null);
        }
        runIntServiceCall(
            call,
            "Failed to request LXMF sync.",
            service -> service.requestLxmfSyncJson(payload.toString())
        );
    }

    @PluginMethod
    public void listAnnounces(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to list announces.",
            "Native announce list JSON parse failed.",
            ReticulumNodeService::listAnnouncesJson
        );
    }

    @PluginMethod
    public void listPeers(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to list peers.",
            "Native peer list JSON parse failed.",
            ReticulumNodeService::listPeersJson
        );
    }

    @PluginMethod
    public void listConversations(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to list conversations.",
            "Native conversation list JSON parse failed.",
            ReticulumNodeService::listConversationsJson
        );
    }

    @PluginMethod
    public void listMessages(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("conversationId", call.getString("conversationId"));
        runStringServiceCall(
            call,
            "Failed to list messages.",
            "Native message list JSON parse failed.",
            service -> service.listMessagesJson(payload.toString())
        );
    }

    @PluginMethod
    public void getLxmfSyncStatus(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to get LXMF sync status.",
            "Native sync status JSON parse failed.",
            ReticulumNodeService::getLxmfSyncStatusJson
        );
    }

    @PluginMethod
    public void legacyImportCompleted(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to read legacy import state.",
            "Native legacy import JSON parse failed.",
            ReticulumNodeService::legacyImportCompletedJson
        );
    }

    @PluginMethod
    public void importLegacyState(PluginCall call) {
        final JSObject payload = call.getObject("payload", new JSObject());
        runIntServiceCall(
            call,
            "Failed to import legacy state.",
            service -> service.importLegacyStateJson(payload.toString())
        );
    }

    @PluginMethod
    public void getAppSettings(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to get app settings.",
            "Native app settings JSON parse failed.",
            ReticulumNodeService::getAppSettingsJson
        );
    }

    @PluginMethod
    public void setAppSettings(PluginCall call) {
        final JSObject payload = call.getObject("settings", new JSObject());
        runIntServiceCall(
            call,
            "Failed to save app settings.",
            service -> service.setAppSettingsJson(payload.toString())
        );
    }

    @PluginMethod
    public void getSavedPeers(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to get saved peers.",
            "Native saved peers JSON parse failed.",
            ReticulumNodeService::getSavedPeersJson
        );
    }

    @PluginMethod
    public void setSavedPeers(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("savedPeers", call.getArray("savedPeers"));
        runIntServiceCall(
            call,
            "Failed to save peers.",
            service -> service.setSavedPeersJson(payload.toString())
        );
    }

    @PluginMethod
    public void getOperationalSummary(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to get operational summary.",
            "Native operational summary JSON parse failed.",
            ReticulumNodeService::getOperationalSummaryJson
        );
    }

    @PluginMethod
    public void getEams(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to get EAMs.",
            "Native EAM JSON parse failed.",
            ReticulumNodeService::getEamsJson
        );
    }

    @PluginMethod
    public void upsertEam(PluginCall call) {
        final JSObject payload = call.getObject("eam", new JSObject());
        runIntServiceCall(
            call,
            "Failed to save EAM.",
            service -> service.upsertEamJson(payload.toString())
        );
    }

    @PluginMethod
    public void deleteEam(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("callsign", call.getString("callsign"));
        final Long deletedAtMs = call.getLong("deletedAtMs");
        if (deletedAtMs != null) {
            payload.put("deletedAtMs", deletedAtMs);
        }
        runIntServiceCall(
            call,
            "Failed to delete EAM.",
            service -> service.deleteEamJson(payload.toString())
        );
    }

    @PluginMethod
    public void getEamTeamSummary(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("teamUid", call.getString("teamUid"));
        runStringServiceCall(
            call,
            "Failed to get EAM team summary.",
            "Native EAM team summary JSON parse failed.",
            service -> service.getEamTeamSummaryJson(payload.toString())
        );
    }

    @PluginMethod
    public void getEvents(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to get events.",
            "Native events JSON parse failed.",
            ReticulumNodeService::getEventsJson
        );
    }

    @PluginMethod
    public void upsertEvent(PluginCall call) {
        final JSObject payload = call.getObject("event", new JSObject());
        runIntServiceCall(
            call,
            "Failed to save event.",
            service -> service.upsertEventJson(payload.toString())
        );
    }

    @PluginMethod
    public void deleteEvent(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("uid", call.getString("uid"));
        final Long deletedAtMs = call.getLong("deletedAtMs");
        if (deletedAtMs != null) {
            payload.put("deletedAtMs", deletedAtMs);
        }
        runIntServiceCall(
            call,
            "Failed to delete event.",
            service -> service.deleteEventJson(payload.toString())
        );
    }

    @PluginMethod
    public void getTelemetryPositions(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to get telemetry positions.",
            "Native telemetry JSON parse failed.",
            ReticulumNodeService::getTelemetryPositionsJson
        );
    }

    @PluginMethod
    public void recordLocalTelemetryFix(PluginCall call) {
        final JSObject payload = call.getObject("position", new JSObject());
        runIntServiceCall(
            call,
            "Failed to record local telemetry.",
            service -> service.recordLocalTelemetryFixJson(payload.toString())
        );
    }

    @PluginMethod
    public void deleteLocalTelemetry(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("callsign", call.getString("callsign"));
        runIntServiceCall(
            call,
            "Failed to delete local telemetry.",
            service -> service.deleteLocalTelemetryJson(payload.toString())
        );
    }

    @PluginMethod
    public void removeAllListeners(PluginCall call) {
        call.resolve();
    }

    private void bindToService() {
        if (serviceBound) {
            return;
        }
        final Context appContext = getContext().getApplicationContext();
        final Intent serviceIntent = new Intent(appContext, ReticulumNodeService.class);
        final boolean bound = appContext.bindService(serviceIntent, serviceConnection, Context.BIND_AUTO_CREATE);
        if (!bound) {
            Logger.error(TAG, "Failed to bind ReticulumNodeService.", null);
        }
    }

    private void unbindFromService() {
        if (!serviceBound) {
            return;
        }
        final Context appContext = getContext().getApplicationContext();
        appContext.unbindService(serviceConnection);
        serviceBound = false;
        boundService = null;
        resetServiceFuture();
    }

    private void startServiceForRuntime() {
        final Context appContext = getContext().getApplicationContext();
        final Intent serviceIntent = new Intent(appContext, ReticulumNodeService.class);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            ContextCompat.startForegroundService(appContext, serviceIntent);
        } else {
            appContext.startService(serviceIntent);
        }
        bindToService();
    }

    private ReticulumNodeService awaitService() throws Exception {
        bindToService();
        return serviceFuture.get(SERVICE_BIND_TIMEOUT_MS, TimeUnit.MILLISECONDS);
    }

    private void tryRegisterServiceListener() {
        if (boundService == null || serviceListenerRegistered) {
            return;
        }
        boundService.addListener(serviceEventListener);
        serviceListenerRegistered = true;
    }

    private void unregisterServiceListener() {
        if (boundService == null || !serviceListenerRegistered) {
            return;
        }
        boundService.removeListener(serviceEventListener);
        serviceListenerRegistered = false;
    }

    private void resetServiceFuture() {
        serviceFuture = new CompletableFuture<>();
    }

    private void runIntServiceCall(
        PluginCall call,
        String fallbackMessage,
        ServiceIntOperation operation
    ) {
        runIntServiceCall(call, fallbackMessage, operation, null);
    }

    private void runIntServiceCall(
        PluginCall call,
        String fallbackMessage,
        ServiceIntOperation operation,
        Runnable onSuccess
    ) {
        bridgeExecutor.execute(() -> {
            try {
                final ReticulumNodeService service = awaitService();
                final int result = operation.run(service);
                if (result != 0) {
                    rejectFromNative(call, fallbackMessage);
                    return;
                }
                if (onSuccess != null) {
                    onSuccess.run();
                }
                call.resolve();
            } catch (Exception ex) {
                call.reject(fallbackMessage, ex);
            }
        });
    }

    private void runStringServiceCall(
        PluginCall call,
        String fallbackMessage,
        String parseFallbackMessage,
        ServiceStringOperation operation
    ) {
        bridgeExecutor.execute(() -> {
            try {
                final ReticulumNodeService service = awaitService();
                final String raw = operation.run(service);
                if (raw == null || raw.isEmpty()) {
                    rejectFromNative(call, fallbackMessage);
                    return;
                }
                resolveJson(call, raw, parseFallbackMessage);
            } catch (Exception ex) {
                call.reject(fallbackMessage, ex);
            }
        });
    }

    private interface ServiceIntOperation {
        int run(ReticulumNodeService service) throws Exception;
    }

    private interface ServiceStringOperation {
        String run(ReticulumNodeService service) throws Exception;
    }

    private void mirrorEventToLogcat(String eventName, JSObject payload) {
        if ("log".equals(eventName)) {
            final String level = payload.getString("level", "Info");
            final String message = payload.getString("message", payload.toString());
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

    private void rejectFromNative(PluginCall call, String fallbackMessage) {
        final String raw = ReticulumBridge.takeLastErrorJson();
        if (raw == null || raw.isEmpty()) {
            Logger.error(TAG, fallbackMessage, new Exception(fallbackMessage));
            call.reject(fallbackMessage);
            return;
        }

        try {
            final JSObject payload = new JSObject(raw);
            final String code = payload.getString("code", "NativeError");
            final String message = payload.getString("message", fallbackMessage);
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
}
