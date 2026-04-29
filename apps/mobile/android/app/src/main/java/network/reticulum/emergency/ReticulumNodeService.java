package network.reticulum.emergency;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.os.Binder;
import android.os.Build;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.provider.Settings;
import android.util.Log;

import androidx.core.app.NotificationCompat;
import androidx.core.app.NotificationManagerCompat;

import com.getcapacitor.JSObject;
import com.getcapacitor.Logger;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.io.File;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashSet;
import java.util.Iterator;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import java.util.concurrent.CopyOnWriteArraySet;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicBoolean;

public final class ReticulumNodeService extends Service {
    public interface ServiceEventListener {
        void onNodeEvent(String eventName, JSObject payload);
    }

    public final class LocalBinder extends Binder {
        public ReticulumNodeService getService() {
            return ReticulumNodeService.this;
        }
    }

    private static final String TAG = "ReticulumNodeService";
    private static final String PREFS_NAME = "reticulum-node-service";
    private static final String PREF_DESIRED_RUNNING = "desiredRunning";
    private static final String PREF_LAST_CONFIG = "lastConfig";
    private static final String PREF_LAST_BOOT_COUNT = "lastBootCount";
    static final String ACTION_RESTORE_AFTER_BOOT = "network.reticulum.emergency.action.RESTORE_AFTER_BOOT";
    private static final String ACTION_STOP_SERVICE = "network.reticulum.emergency.action.STOP_NODE";
    private static final String RUNTIME_CHANNEL_ID = "mesh-runtime";
    private static final String RUNTIME_UPDATES_CHANNEL_ID = "operational-updates";
    private static final String SOS_CHANNEL_ID = "sos-emergency";
    private static final int FOREGROUND_NOTIFICATION_ID = 41001;
    private static final int SOS_NOTIFICATION_ID = 41002;
    private static final int BACKGROUND_NOTIFICATION_BASE_ID = 47000;

    private final IBinder binder = new LocalBinder();
    private final CopyOnWriteArraySet<ServiceEventListener> listeners = new CopyOnWriteArraySet<>();
    private final AtomicBoolean pollerRunning = new AtomicBoolean(false);
    private final ExecutorService pollerExecutor = Executors.newSingleThreadExecutor();
    private final Handler mainHandler = new Handler(Looper.getMainLooper());

    private SharedPreferences preferences;
    private String storageDir = "";
    private String lastResolvedConfigJson = "";
    private String lastCanonicalConfigJson = "";
    private String latestStatusJson = "";
    private String latestSyncStatusJson = "";
    private String latestSosStatusJson = "";
    private SosPlatformCoordinator sosPlatformCoordinator;
    private final Set<String> seenEamKeys = new HashSet<>();
    private final Set<String> seenEventKeys = new HashSet<>();
    private final Set<String> seenChecklistKeys = new HashSet<>();
    private final Set<String> seenMessageIds = new HashSet<>();
    private int nextBackgroundNotificationId = BACKGROUND_NOTIFICATION_BASE_ID;

    @Override
    public void onCreate() {
        super.onCreate();
        preferences = getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE);
        storageDir = resolveStorageDir("").getAbsolutePath();
        initializeBridgeStorage(storageDir);
        createNotificationChannels();
        sosPlatformCoordinator = new SosPlatformCoordinator(this);
        latestStatusJson = safeStatusJson();
        latestSyncStatusJson = safeSyncStatusJson();
        latestSosStatusJson = safeSosStatusJson();
        applyCurrentSosPlatformSettings();
        maybeRestoreAfterProcessRecreation();
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent != null && ACTION_STOP_SERVICE.equals(intent.getAction())) {
            stopNode();
            return START_NOT_STICKY;
        }
        if (intent != null && ACTION_RESTORE_AFTER_BOOT.equals(intent.getAction())) {
            maybeRestoreAfterBoot();
            return START_STICKY;
        }

        if (shouldBeRunning() && !isNodeRunning()) {
            maybeRestoreAfterProcessRecreation();
        }
        return START_STICKY;
    }

    @Override
    public IBinder onBind(Intent intent) {
        return binder;
    }

    @Override
    public boolean onUnbind(Intent intent) {
        return true;
    }

    @Override
    public void onDestroy() {
        stopPoller();
        if (sosPlatformCoordinator != null) {
            sosPlatformCoordinator.close();
        }
        pollerExecutor.shutdownNow();
        super.onDestroy();
    }

    @Override
    public void onTaskRemoved(Intent rootIntent) {
        super.onTaskRemoved(rootIntent);
    }

    public void addListener(ServiceEventListener listener) {
        if (listener == null) {
            return;
        }
        listeners.add(listener);
        mainHandler.post(() -> {
            emitCachedState(listener);
            emitProjectionRefreshSweep(listener);
        });
    }

    public void removeListener(ServiceEventListener listener) {
        if (listener == null) {
            return;
        }
        listeners.remove(listener);
    }

    public synchronized int startNode(String configJson) {
        try {
            final ResolvedConfig resolved = resolveConfig(configJson);
            initializeBridgeStorage(resolved.storageDir);
            if (isNodeRunning()) {
                if (resolved.canonicalConfig.equals(lastCanonicalConfigJson)) {
                    persistDesiredRunning(true, resolved);
                    ensurePoller();
                    refreshLatestRuntimeState();
                    startForeground(FOREGROUND_NOTIFICATION_ID, buildRuntimeNotification(true));
                    emitCachedStateToAll();
                    emitProjectionRefreshSweepToAll();
                    return 0;
                }
                final int restartResult = ReticulumBridge.restart(resolved.resolvedJson);
                if (restartResult != 0) {
                    return restartResult;
                }
            } else {
                promoteServiceForRuntime();
                final int startResult = ReticulumBridge.start(resolved.resolvedJson);
                if (startResult != 0) {
                    stopForeground(STOP_FOREGROUND_REMOVE);
                    return startResult;
                }
            }

            lastResolvedConfigJson = resolved.resolvedJson;
            lastCanonicalConfigJson = resolved.canonicalConfig;
            persistDesiredRunning(true, resolved);
            primeOperationalNotificationState();
            refreshLatestRuntimeState();
            ensurePoller();
            startForeground(FOREGROUND_NOTIFICATION_ID, buildRuntimeNotification(true));
            emitCachedStateToAll();
            emitProjectionRefreshSweepToAll();
            return 0;
        } catch (Exception ex) {
            Logger.error(TAG, "Failed to start node", ex);
            return -1;
        }
    }

    public synchronized int stopNode() {
        stopPoller();
        final int result = ReticulumBridge.stop();
        clearDesiredRunning();
        refreshLatestRuntimeState();
        emitCachedStateToAll();
        stopForeground(STOP_FOREGROUND_REMOVE);
        stopSelf();
        return result;
    }

    public synchronized int restartNode(String configJson) {
        try {
            final ResolvedConfig resolved = resolveConfig(configJson);
            promoteServiceForRuntime();
            final int result = ReticulumBridge.restart(resolved.resolvedJson);
            if (result != 0) {
                return result;
            }

            lastResolvedConfigJson = resolved.resolvedJson;
            lastCanonicalConfigJson = resolved.canonicalConfig;
            persistDesiredRunning(true, resolved);
            primeOperationalNotificationState();
            refreshLatestRuntimeState();
            ensurePoller();
            startForeground(FOREGROUND_NOTIFICATION_ID, buildRuntimeNotification(true));
            emitCachedStateToAll();
            emitProjectionRefreshSweepToAll();
            return 0;
        } catch (Exception ex) {
            Logger.error(TAG, "Failed to restart node", ex);
            return -1;
        }
    }

    public synchronized String getStatusJson() {
        return nonEmptyJson(ReticulumBridge.getStatusJson(), "{}");
    }

    public synchronized int connectPeer(String destinationHex) {
        return ReticulumBridge.connectPeer(destinationHex);
    }

    public synchronized int disconnectPeer(String destinationHex) {
        return ReticulumBridge.disconnectPeer(destinationHex);
    }

    public synchronized int announceNow() {
        return ReticulumBridge.announceNow();
    }

    public synchronized int requestPeerIdentity(String destinationHex) {
        return ReticulumBridge.requestPeerIdentity(destinationHex);
    }

    public synchronized int sendJson(String payloadJson) {
        return ReticulumBridge.sendJson(payloadJson);
    }

    public synchronized String sendLxmfJson(String payloadJson) {
        return nonEmptyJson(ReticulumBridge.sendLxmfJson(payloadJson), "");
    }

    public synchronized int retryLxmfJson(String payloadJson) {
        return ReticulumBridge.retryLxmfJson(payloadJson);
    }

    public synchronized int cancelLxmfJson(String payloadJson) {
        return ReticulumBridge.cancelLxmfJson(payloadJson);
    }

    public synchronized int broadcastBase64(String bytesBase64) {
        return ReticulumBridge.broadcastBase64(bytesBase64);
    }

    public synchronized int setActivePropagationNodeJson(String payloadJson) {
        return ReticulumBridge.setActivePropagationNodeJson(payloadJson);
    }

    public synchronized int requestLxmfSyncJson(String payloadJson) {
        return ReticulumBridge.requestLxmfSyncJson(payloadJson);
    }

    public synchronized String listAnnouncesJson() {
        return nonEmptyJson(ReticulumBridge.listAnnouncesJson(), "{\"items\":[]}");
    }

    public synchronized String getPluginsJson() {
        return nonEmptyJson(ReticulumBridge.getPluginsJson(primaryAndroidAbi()), "{\"items\":[],\"errors\":[]}");
    }

    public synchronized String installPluginPackageJson(String payloadJson) {
        return ReticulumBridge.installPluginPackageJson(primaryAndroidAbi(), payloadJson);
    }

    public synchronized int setPluginEnabledJson(String payloadJson) {
        return ReticulumBridge.setPluginEnabledJson(primaryAndroidAbi(), payloadJson);
    }

    public synchronized int grantPluginPermissionsJson(String payloadJson) {
        return ReticulumBridge.grantPluginPermissionsJson(primaryAndroidAbi(), payloadJson);
    }

    public synchronized String listPeersJson() {
        return nonEmptyJson(ReticulumBridge.listPeersJson(), "{\"items\":[]}");
    }

    public synchronized String listConversationsJson() {
        return nonEmptyJson(ReticulumBridge.listConversationsJson(), "{\"items\":[]}");
    }

    public synchronized String listMessagesJson(String payloadJson) {
        return nonEmptyJson(ReticulumBridge.listMessagesJson(payloadJson), "{\"items\":[]}");
    }

    public synchronized int deleteConversationJson(String payloadJson) {
        return ReticulumBridge.deleteConversationJson(payloadJson);
    }

    public synchronized String getLxmfSyncStatusJson() {
        return nonEmptyJson(ReticulumBridge.getLxmfSyncStatusJson(), "{}");
    }

    public synchronized String listTelemetryDestinationsJson() {
        return nonEmptyJson(ReticulumBridge.listTelemetryDestinationsJson(), "{\"items\":[]}");
    }

    public synchronized String legacyImportCompletedJson() {
        return nonEmptyJson(ReticulumBridge.legacyImportCompletedJson(), "{\"completed\":false}");
    }

    public synchronized int importLegacyStateJson(String payloadJson) {
        return ReticulumBridge.importLegacyStateJson(payloadJson);
    }

    public synchronized String getAppSettingsJson() {
        return nonEmptyJson(ReticulumBridge.getAppSettingsJson(), "{}");
    }

    public synchronized int setAppSettingsJson(String payloadJson) {
        return ReticulumBridge.setAppSettingsJson(payloadJson);
    }

    public synchronized String getSavedPeersJson() {
        return nonEmptyJson(ReticulumBridge.getSavedPeersJson(), "{\"items\":[]}");
    }

    public synchronized int setSavedPeersJson(String payloadJson) {
        return ReticulumBridge.setSavedPeersJson(payloadJson);
    }

    public synchronized String getOperationalSummaryJson() {
        return nonEmptyJson(ReticulumBridge.getOperationalSummaryJson(), "{}");
    }

    public synchronized String getChecklistsJson(String payloadJson) {
        return nonEmptyJson(ReticulumBridge.getChecklistsJson(payloadJson), "{\"items\":[]}");
    }

    public synchronized String getChecklistJson(String payloadJson) {
        return nonEmptyJson(ReticulumBridge.getChecklistJson(payloadJson), "{}");
    }

    public synchronized String getChecklistTemplatesJson(String payloadJson) {
        return nonEmptyJson(ReticulumBridge.getChecklistTemplatesJson(payloadJson), "{\"items\":[]}");
    }

    public synchronized String importChecklistTemplateCsvJson(String payloadJson) {
        return nonEmptyJson(ReticulumBridge.importChecklistTemplateCsvJson(payloadJson), "{}");
    }

    public synchronized int createChecklistFromTemplateJson(String payloadJson) {
        return ReticulumBridge.createChecklistFromTemplateJson(payloadJson);
    }

    public synchronized int createOnlineChecklistJson(String payloadJson) {
        return ReticulumBridge.createOnlineChecklistJson(payloadJson);
    }

    public synchronized int updateChecklistJson(String payloadJson) {
        return ReticulumBridge.updateChecklistJson(payloadJson);
    }

    public synchronized int deleteChecklistJson(String payloadJson) {
        return ReticulumBridge.deleteChecklistJson(payloadJson);
    }

    public synchronized int joinChecklistJson(String payloadJson) {
        return ReticulumBridge.joinChecklistJson(payloadJson);
    }

    public synchronized int uploadChecklistJson(String payloadJson) {
        return ReticulumBridge.uploadChecklistJson(payloadJson);
    }

    public synchronized int setChecklistTaskStatusJson(String payloadJson) {
        return ReticulumBridge.setChecklistTaskStatusJson(payloadJson);
    }

    public synchronized int addChecklistTaskRowJson(String payloadJson) {
        return ReticulumBridge.addChecklistTaskRowJson(payloadJson);
    }

    public synchronized int deleteChecklistTaskRowJson(String payloadJson) {
        return ReticulumBridge.deleteChecklistTaskRowJson(payloadJson);
    }

    public synchronized int setChecklistTaskRowStyleJson(String payloadJson) {
        return ReticulumBridge.setChecklistTaskRowStyleJson(payloadJson);
    }

    public synchronized int setChecklistTaskCellJson(String payloadJson) {
        return ReticulumBridge.setChecklistTaskCellJson(payloadJson);
    }

    public synchronized String getEamsJson() {
        return nonEmptyJson(ReticulumBridge.getEamsJson(), "{\"items\":[]}");
    }

    public synchronized int upsertEamJson(String payloadJson) {
        return ReticulumBridge.upsertEamJson(payloadJson);
    }

    public synchronized int deleteEamJson(String payloadJson) {
        return ReticulumBridge.deleteEamJson(payloadJson);
    }

    public synchronized String getEamTeamSummaryJson(String payloadJson) {
        return nonEmptyJson(ReticulumBridge.getEamTeamSummaryJson(payloadJson), "{}");
    }

    public synchronized String getEventsJson() {
        return nonEmptyJson(ReticulumBridge.getEventsJson(), "{\"items\":[]}");
    }

    public synchronized int upsertEventJson(String payloadJson) {
        return ReticulumBridge.upsertEventJson(payloadJson);
    }

    public synchronized int deleteEventJson(String payloadJson) {
        return ReticulumBridge.deleteEventJson(payloadJson);
    }

    public synchronized String getTelemetryPositionsJson() {
        return nonEmptyJson(ReticulumBridge.getTelemetryPositionsJson(), "{\"items\":[]}");
    }

    public synchronized int recordLocalTelemetryFixJson(String payloadJson) {
        return ReticulumBridge.recordLocalTelemetryFixJson(payloadJson);
    }

    public synchronized int deleteLocalTelemetryJson(String payloadJson) {
        return ReticulumBridge.deleteLocalTelemetryJson(payloadJson);
    }

    public synchronized String getSosSettingsJson() {
        return nonEmptyJson(ReticulumBridge.getSosSettingsJson(), "{}");
    }

    public synchronized int setSosSettingsJson(String payloadJson) {
        final int result = ReticulumBridge.setSosSettingsJson(payloadJson);
        if (result == 0) {
            applyCurrentSosPlatformSettings();
        }
        return result;
    }

    public synchronized int setSosPinJson(String payloadJson) {
        return ReticulumBridge.setSosPinJson(payloadJson);
    }

    public synchronized String getSosStatusJson() {
        return nonEmptyJson(ReticulumBridge.getSosStatusJson(), "{}");
    }

    public synchronized String triggerSosJson(String payloadJson) {
        if (sosPlatformCoordinator != null) {
            sosPlatformCoordinator.submitTelemetrySnapshot();
        }
        return nonEmptyJson(ReticulumBridge.triggerSosJson(payloadJson), "{}");
    }

    public synchronized String deactivateSosJson(String payloadJson) {
        return nonEmptyJson(ReticulumBridge.deactivateSosJson(payloadJson), "{}");
    }

    public synchronized int submitSosTelemetryJson(String payloadJson) {
        return ReticulumBridge.submitSosTelemetryJson(payloadJson);
    }

    public synchronized String submitSosAccelerometerJson(String payloadJson) {
        return nonEmptyJson(ReticulumBridge.submitSosAccelerometerJson(payloadJson), "{\"triggered\":false}");
    }

    public synchronized String submitSosScreenEventJson(String payloadJson) {
        return nonEmptyJson(ReticulumBridge.submitSosScreenEventJson(payloadJson), "{\"triggered\":false}");
    }

    public synchronized String listSosAlertsJson() {
        return nonEmptyJson(ReticulumBridge.listSosAlertsJson(), "{\"items\":[]}");
    }

    public synchronized String listSosLocationsJson() {
        return nonEmptyJson(ReticulumBridge.listSosLocationsJson(), "{\"items\":[]}");
    }

    public synchronized String listSosAudioJson() {
        return nonEmptyJson(ReticulumBridge.listSosAudioJson(), "{\"items\":[]}");
    }

    public synchronized int setAnnounceCapabilities(String capabilityString) {
        return ReticulumBridge.setAnnounceCapabilities(capabilityString);
    }

    public synchronized int setLogLevel(String levelString) {
        return ReticulumBridge.setLogLevel(levelString);
    }

    public synchronized int refreshHubDirectory() {
        return ReticulumBridge.refreshHubDirectory();
    }

    public synchronized String takeLastErrorJson() {
        return ReticulumBridge.takeLastErrorJson();
    }

    private void maybeRestoreAfterProcessRecreation() {
        if (!shouldBeRunning()) {
            return;
        }

        final String persistedConfig = preferences.getString(PREF_LAST_CONFIG, "");
        if (persistedConfig == null || persistedConfig.trim().isEmpty()) {
            return;
        }

        if (isNodeRunning()) {
            ensurePoller();
            refreshLatestRuntimeState();
            startForeground(FOREGROUND_NOTIFICATION_ID, buildRuntimeNotification(true));
            return;
        }

        final int result = startNode(persistedConfig);
        if (result != 0) {
            Log.e(TAG, "Failed to restore node after process recreation");
        }
    }

    private boolean shouldBeRunning() {
        if (!preferences.getBoolean(PREF_DESIRED_RUNNING, false)) {
            return false;
        }
        return preferences.getInt(PREF_LAST_BOOT_COUNT, -1) == currentBootCount();
    }

    private boolean isNodeRunning() {
        try {
            final JSONObject payload = new JSONObject(nonEmptyJson(ReticulumBridge.getStatusJson(), "{}"));
            return payload.optBoolean("running", false);
        } catch (JSONException ex) {
            return false;
        }
    }

    private void persistDesiredRunning(boolean desiredRunning, ResolvedConfig resolved) {
        preferences.edit()
            .putBoolean(PREF_DESIRED_RUNNING, desiredRunning)
            .putString(PREF_LAST_CONFIG, resolved.resolvedJson)
            .putInt(PREF_LAST_BOOT_COUNT, currentBootCount())
            .apply();
    }

    private void clearDesiredRunning() {
        preferences.edit()
            .putBoolean(PREF_DESIRED_RUNNING, false)
            .remove(PREF_LAST_CONFIG)
            .putInt(PREF_LAST_BOOT_COUNT, currentBootCount())
            .apply();
        lastResolvedConfigJson = "";
        lastCanonicalConfigJson = "";
    }

    private void initializeBridgeStorage(String resolvedStorageDir) {
        storageDir = resolvedStorageDir;
        final int result = ReticulumBridge.initializeStorage(resolvedStorageDir);
        if (result != 0) {
            Logger.error(
                TAG,
                "Failed to initialize bridge storage: " + nonEmptyJson(ReticulumBridge.takeLastErrorJson(), "unknown"),
                null
            );
        }
    }

    private void ensurePoller() {
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

    private void stopPoller() {
        pollerRunning.set(false);
    }

    private void handleNativeEvent(String eventName, JSObject payload) {
        if (eventName == null || eventName.isEmpty()) {
            return;
        }
        mirrorEventToLogcat(eventName, payload);
        updateCachedState(eventName, payload);
        dispatchEventToListeners(eventName, payload);
        if (listeners.isEmpty()) {
            maybeNotifyInboundUpdate(eventName, payload);
        }
        if ("sosTelemetryRequested".equals(eventName) && sosPlatformCoordinator != null) {
            sosPlatformCoordinator.submitTelemetrySnapshot();
        }
        if ("statusChanged".equals(eventName) || "syncUpdated".equals(eventName)) {
            updateForegroundNotification();
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
            } catch (JSONException ignored) {
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
            } catch (JSONException ignored) {
                latestSosStatusJson = payload.toString();
            }
        }
    }

    private void maybeRestoreAfterBoot() {
        if (!preferences.getBoolean(PREF_DESIRED_RUNNING, false)) {
            return;
        }
        final String persistedConfig = preferences.getString(PREF_LAST_CONFIG, "");
        if (persistedConfig == null || persistedConfig.trim().isEmpty()) {
            return;
        }
        final int result = startNode(persistedConfig);
        if (result != 0) {
            Log.e(TAG, "Failed to restore node after boot");
        }
    }

    private void dispatchEventToListeners(String eventName, JSObject payload) {
        for (ServiceEventListener listener : listeners) {
            mainHandler.post(() -> listener.onNodeEvent(eventName, payload));
        }
    }

    private void emitCachedState(ServiceEventListener listener) {
        if (listener == null) {
            return;
        }
        try {
            final JSObject statusPayload = new JSObject();
            statusPayload.put("status", new JSObject(nonEmptyJson(latestStatusJson, "{}")));
            listener.onNodeEvent("statusChanged", statusPayload);
        } catch (JSONException ignored) {
            listener.onNodeEvent("statusChanged", new JSObject());
        }

        try {
            listener.onNodeEvent("syncUpdated", new JSObject(nonEmptyJson(latestSyncStatusJson, "{}")));
        } catch (JSONException ignored) {
            listener.onNodeEvent("syncUpdated", new JSObject());
        }

        try {
            final JSObject statusPayload = new JSObject();
            statusPayload.put("status", new JSObject(nonEmptyJson(latestSosStatusJson, "{}")));
            listener.onNodeEvent("sosStatusChanged", statusPayload);
        } catch (JSONException ignored) {
            listener.onNodeEvent("sosStatusChanged", new JSObject());
        }
    }

    private void emitCachedStateToAll() {
        for (ServiceEventListener listener : listeners) {
            emitCachedState(listener);
        }
    }

    private void emitProjectionRefreshSweep(ServiceEventListener listener) {
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
            "Plugins",
            "Sos",
        }) {
            final JSObject payload = new JSObject();
            payload.put("scope", scope);
            payload.put("revision", 0);
            payload.put("updatedAtMs", System.currentTimeMillis());
            payload.put("reason", "serviceRebind");
            listener.onNodeEvent("projectionInvalidated", payload);
        }
    }

    private void emitProjectionRefreshSweepToAll() {
        for (ServiceEventListener listener : listeners) {
            emitProjectionRefreshSweep(listener);
        }
    }

    private void refreshLatestRuntimeState() {
        latestStatusJson = safeStatusJson();
        latestSyncStatusJson = safeSyncStatusJson();
        latestSosStatusJson = safeSosStatusJson();
        applyCurrentSosPlatformSettings();
    }

    private String safeStatusJson() {
        return nonEmptyJson(ReticulumBridge.getStatusJson(), "{}");
    }

    private String safeSyncStatusJson() {
        return nonEmptyJson(ReticulumBridge.getLxmfSyncStatusJson(), "{}");
    }

    private String safeSosStatusJson() {
        return nonEmptyJson(ReticulumBridge.getSosStatusJson(), "{}");
    }

    private void applyCurrentSosPlatformSettings() {
        if (sosPlatformCoordinator != null) {
            sosPlatformCoordinator.applySettingsJson(nonEmptyJson(ReticulumBridge.getSosSettingsJson(), "{}"));
        }
    }

    private String nonEmptyJson(String raw, String fallback) {
        if (raw == null || raw.trim().isEmpty()) {
            return fallback;
        }
        return raw;
    }

    private void createNotificationChannels() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) {
            return;
        }

        final NotificationManager manager = getSystemService(NotificationManager.class);
        if (manager == null) {
            return;
        }

        final NotificationChannel runtimeChannel = new NotificationChannel(
            RUNTIME_CHANNEL_ID,
            "Mesh Runtime",
            NotificationManager.IMPORTANCE_LOW
        );
        runtimeChannel.setDescription("Foreground Reticulum mesh runtime");

        final NotificationChannel updatesChannel = new NotificationChannel(
            RUNTIME_UPDATES_CHANNEL_ID,
            "Operational Updates",
            NotificationManager.IMPORTANCE_DEFAULT
        );
        updatesChannel.setDescription("Incoming mesh events, action messages, and chat");

        final NotificationChannel sosChannel = new NotificationChannel(
            SOS_CHANNEL_ID,
            "SOS Emergency",
            NotificationManager.IMPORTANCE_HIGH
        );
        sosChannel.setDescription("Urgent SOS alerts received over the mesh");

        manager.createNotificationChannel(runtimeChannel);
        manager.createNotificationChannel(updatesChannel);
        manager.createNotificationChannel(sosChannel);
    }

    private void promoteServiceForRuntime() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            startForeground(FOREGROUND_NOTIFICATION_ID, buildRuntimeNotification(false));
        }
    }

    private Notification buildRuntimeNotification(boolean running) {
        final Intent launchIntent = new Intent(this, MainActivity.class);
        launchIntent.addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP | Intent.FLAG_ACTIVITY_NEW_TASK);
        final PendingIntent contentIntent = PendingIntent.getActivity(
            this,
            0,
            launchIntent,
            PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE
        );

        final Intent stopIntent = new Intent(this, ReticulumNodeService.class);
        stopIntent.setAction(ACTION_STOP_SERVICE);
        final PendingIntent stopPendingIntent = PendingIntent.getService(
            this,
            1,
            stopIntent,
            PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE
        );

        final String title = running ? "Mesh node running" : "Starting mesh node";
        final String body = buildRuntimeNotificationBody(running);

        return new NotificationCompat.Builder(this, RUNTIME_CHANNEL_ID)
            .setContentTitle(title)
            .setContentText(body)
            .setStyle(new NotificationCompat.BigTextStyle().bigText(body))
            .setSmallIcon(R.mipmap.ic_launcher)
            .setOngoing(running)
            .setOnlyAlertOnce(true)
            .setContentIntent(contentIntent)
            .addAction(0, "Stop", stopPendingIntent)
            .build();
    }

    private String buildRuntimeNotificationBody(boolean running) {
        if (!running) {
            return "Bringing the Reticulum node online";
        }
        try {
            final JSONObject status = new JSONObject(nonEmptyJson(latestStatusJson, "{}"));
            final JSONObject sync = new JSONObject(nonEmptyJson(latestSyncStatusJson, "{}"));
            final String name = status.optString("name", getString(R.string.app_name));
            final String phase = sync.optString("phase", "Idle");
            return name + " | Sync " + phase;
        } catch (JSONException ex) {
            return getString(R.string.app_name);
        }
    }

    private void updateForegroundNotification() {
        if (!isNodeRunning()) {
            return;
        }
        NotificationManagerCompat.from(this).notify(
            FOREGROUND_NOTIFICATION_ID,
            buildRuntimeNotification(true)
        );
    }

    private void maybeNotifyInboundUpdate(String eventName, JSObject payload) {
        if ("sosAlertChanged".equals(eventName)) {
            maybeNotifySosAlert(payload);
            return;
        }
        if ("messageReceived".equals(eventName) || "messageUpdated".equals(eventName)) {
            maybeNotifyInboundMessage(payload);
            return;
        }
        if ("projectionInvalidated".equals(eventName)) {
            final String scope = payload.getString("scope", "");
            if ("Eams".equals(scope)) {
                maybeNotifyInboundEams();
            } else if ("Events".equals(scope)) {
                maybeNotifyInboundEvents();
            } else if ("Checklists".equals(scope)) {
                maybeNotifyInboundChecklists();
            }
        }
    }

    private void maybeNotifySosAlert(JSObject payload) {
        final JSONObject nestedAlert = payload.optJSONObject("alert");
        final JSONObject alert = nestedAlert == null ? payload : nestedAlert;
        final boolean active = alert.optBoolean("active", true);
        if (!active) {
            NotificationManagerCompat.from(this).cancel(SOS_NOTIFICATION_ID);
            postBackgroundNotification("SOS cancelled", "The sender marked themselves safe.");
            return;
        }
        final String source = alert.optString("sourceHex", "Unknown");
        final String body = truncate(alert.optString("bodyUtf8", "Emergency SOS alert"));
        postSosNotification("SOS EMERGENCY from " + source, body);
    }

    private void maybeNotifyInboundMessage(JSObject payload) {
        final String direction = payload.getString("direction", "");
        final String messageId = payload.getString("messageIdHex", "").trim().toLowerCase();
        if (!"Inbound".equals(direction) || messageId.isEmpty() || !seenMessageIds.add(messageId)) {
            return;
        }
        final String peer = payload.getString("sourceHex", payload.getString("destinationHex", "Unknown"));
        final String body = truncate(payload.getString("bodyUtf8", "(empty message)"));
        postBackgroundNotification("Message from " + peer, body);
    }

    private void maybeNotifyInboundEams() {
        try {
            final JSONObject root = new JSONObject(nonEmptyJson(ReticulumBridge.getEamsJson(), "{\"items\":[]}"));
            final JSONArray items = root.optJSONArray("items");
            if (items == null) {
                return;
            }
            final JSONObject status = new JSONObject(nonEmptyJson(latestStatusJson, "{}"));
            final String localIdentity = status.optString("identityHex", "").trim().toLowerCase();
            final String localAppDestination = status.optString("appDestinationHex", "").trim().toLowerCase();
            final String localName = status.optString("name", "").trim().toLowerCase();

            for (int index = 0; index < items.length(); index += 1) {
                final JSONObject item = items.optJSONObject(index);
                if (item == null || item.has("deletedAt")) {
                    continue;
                }
                final String callsign = item.optString("callsign", "").trim();
                final long updatedAt = item.optLong("updatedAt", 0L);
                if (callsign.isEmpty() || updatedAt <= 0L) {
                    continue;
                }
                final String key = callsign.toLowerCase() + ":" + updatedAt;
                if (!seenEamKeys.add(key)) {
                    continue;
                }

                final String teamMemberUid = item.optString("teamMemberUid", "").trim().toLowerCase();
                final JSONObject source = item.optJSONObject("source");
                final String sourceIdentity = source == null
                    ? ""
                    : source.optString("rnsIdentity", source.optString("rns_identity", "")).trim().toLowerCase(Locale.US);
                final String reportedBy = item.optString("reportedBy", "").trim().toLowerCase();
                if (
                    (!localAppDestination.isEmpty() && localAppDestination.equals(teamMemberUid))
                        || (!localIdentity.isEmpty() && localIdentity.equals(sourceIdentity))
                        || (!localName.isEmpty() && (localName.equals(reportedBy) || localName.equals(callsign.toLowerCase())))
                ) {
                    continue;
                }

                final String title = "EAM from " + item.optString("reportedBy", callsign);
                final String notes = item.optString("notes", "").trim();
                final String body = !notes.isEmpty()
                    ? truncate(notes)
                    : truncate(item.optString("groupName", "Team") + " status " + item.optString("overallStatus", "updated"));
                postBackgroundNotification(title, body);
            }
        } catch (JSONException ignored) {
        }
    }

    private void maybeNotifyInboundEvents() {
        try {
            final JSONObject root = new JSONObject(nonEmptyJson(ReticulumBridge.getEventsJson(), "{\"items\":[]}"));
            final JSONArray items = root.optJSONArray("items");
            if (items == null) {
                return;
            }
            final JSONObject status = new JSONObject(nonEmptyJson(latestStatusJson, "{}"));
            final String localIdentity = status.optString("identityHex", "").trim().toLowerCase();
            final String localName = status.optString("name", "").trim().toLowerCase();

            for (int index = 0; index < items.length(); index += 1) {
                final JSONObject item = items.optJSONObject(index);
                final JSONObject args = item == null ? null : item.optJSONObject("args");
                final JSONObject source = item == null ? null : item.optJSONObject("source");
                if (args == null || source == null || item.has("deleted_at")) {
                    continue;
                }
                final String uid = args.optString("entry_uid", "").trim();
                final long updatedAt = item.optLong("updatedAt", 0L);
                if (uid.isEmpty() || updatedAt <= 0L) {
                    continue;
                }
                final String key = uid.toLowerCase() + ":" + updatedAt;
                if (!seenEventKeys.add(key)) {
                    continue;
                }

                final String sourceIdentity = args.optString(
                    "sourceIdentity",
                    args.optString(
                        "source_identity",
                        source.optString("rnsIdentity", source.optString("rns_identity", ""))
                    )
                ).trim().toLowerCase(Locale.US);
                final String sourceDisplayName = args.optString(
                    "sourceDisplayName",
                    args.optString(
                        "source_display_name",
                        source.optString("displayName", source.optString("display_name", ""))
                    )
                ).trim().toLowerCase(Locale.US);
                final String callsign = args.optString("callsign", "").trim();
                if (
                    (!localIdentity.isEmpty() && localIdentity.equals(sourceIdentity))
                        || (!localName.isEmpty() && (localName.equals(sourceDisplayName) || localName.equals(callsign.toLowerCase())))
                ) {
                    continue;
                }

                postBackgroundNotification(
                    "Event from " + (callsign.isEmpty() ? "Unknown" : callsign),
                    truncate(args.optString("content", "Event updated"))
                );
            }
        } catch (JSONException ignored) {
        }
    }

    private void maybeNotifyInboundChecklists() {
        try {
            final JSONObject root = new JSONObject(nonEmptyJson(
                ReticulumBridge.getChecklistsJson("{\"sortBy\":\"updated_at_desc\"}"),
                "{\"items\":[]}"
            ));
            final JSONArray items = root.optJSONArray("items");
            if (items == null) {
                return;
            }
            final JSONObject status = new JSONObject(nonEmptyJson(latestStatusJson, "{}"));
            final String localIdentity = status.optString("identityHex", "").trim().toLowerCase(Locale.US);

            for (int index = 0; index < items.length(); index += 1) {
                final JSONObject item = items.optJSONObject(index);
                if (item == null || item.has("deletedAt") || item.has("deleted_at")) {
                    continue;
                }
                final String key = checklistNotificationKey(item);
                if (key.isEmpty() || !seenChecklistKeys.add(key)) {
                    continue;
                }
                final String changedBy = optStringAny(
                    item,
                    "lastChangedByTeamMemberRnsIdentity",
                    "last_changed_by_team_member_rns_identity"
                ).trim().toLowerCase(Locale.US);
                final String createdBy = optStringAny(
                    item,
                    "createdByTeamMemberRnsIdentity",
                    "created_by_team_member_rns_identity"
                ).trim().toLowerCase(Locale.US);
                if (
                    !localIdentity.isEmpty()
                        && (localIdentity.equals(changedBy)
                            || (changedBy.isEmpty() && localIdentity.equals(createdBy)))
                ) {
                    continue;
                }

                final JSONObject counts = item.optJSONObject("counts");
                final int pendingCount = optIntAny(counts, "pendingCount", "pending_count", 0);
                final int completeCount = optIntAny(counts, "completeCount", "complete_count", 0);
                final int lateCount = optIntAny(counts, "lateCount", "late_count", 0);
                final JSONArray tasks = item.optJSONArray("tasks");
                final int taskCount = tasks == null ? 0 : tasks.length();
                final String lateSummary = lateCount > 0 ? ", " + lateCount + " late" : "";
                final String taskSummary = taskCount == 1 ? "1 task" : taskCount + " tasks";
                postBackgroundNotification(
                    "Checklist updated: " + item.optString("name", "Checklist"),
                    truncate(pendingCount + " pending, " + completeCount + " complete" + lateSummary + " across " + taskSummary)
                );
            }
        } catch (JSONException ignored) {
        }
    }

    private void postBackgroundNotification(String title, String body) {
        final int notificationId = nextNotificationId();
        final Intent launchIntent = new Intent(this, MainActivity.class);
        launchIntent.addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP | Intent.FLAG_ACTIVITY_NEW_TASK);
        final PendingIntent contentIntent = PendingIntent.getActivity(
            this,
            notificationId,
            launchIntent,
            PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE
        );
        final Notification notification = new NotificationCompat.Builder(this, RUNTIME_UPDATES_CHANNEL_ID)
            .setContentTitle(title)
            .setContentText(body)
            .setStyle(new NotificationCompat.BigTextStyle().bigText(body))
            .setSmallIcon(R.mipmap.ic_launcher)
            .setAutoCancel(true)
            .setOnlyAlertOnce(true)
            .setContentIntent(contentIntent)
            .build();
        NotificationManagerCompat.from(this).notify(notificationId, notification);
    }

    private void postSosNotification(String title, String body) {
        final Intent launchIntent = new Intent(this, MainActivity.class);
        launchIntent.addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP | Intent.FLAG_ACTIVITY_NEW_TASK);
        final PendingIntent contentIntent = PendingIntent.getActivity(
            this,
            SOS_NOTIFICATION_ID,
            launchIntent,
            PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE
        );
        final Notification notification = new NotificationCompat.Builder(this, SOS_CHANNEL_ID)
            .setContentTitle(title)
            .setContentText(body)
            .setStyle(new NotificationCompat.BigTextStyle().bigText(body))
            .setSmallIcon(R.mipmap.ic_launcher)
            .setOngoing(true)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setCategory(NotificationCompat.CATEGORY_ALARM)
            .setContentIntent(contentIntent)
            .addAction(0, "Open Chat", contentIntent)
            .addAction(0, "View on Map", contentIntent)
            .build();
        NotificationManagerCompat.from(this).notify(SOS_NOTIFICATION_ID, notification);
    }

    private void primeOperationalNotificationState() {
        seenMessageIds.clear();
        maybePrimeEamKeys();
        maybePrimeEventKeys();
        maybePrimeChecklistKeys();
    }

    private void maybePrimeEamKeys() {
        seenEamKeys.clear();
        try {
            final JSONObject root = new JSONObject(nonEmptyJson(ReticulumBridge.getEamsJson(), "{\"items\":[]}"));
            final JSONArray items = root.optJSONArray("items");
            if (items == null) {
                return;
            }
            for (int index = 0; index < items.length(); index += 1) {
                final JSONObject item = items.optJSONObject(index);
                if (item == null || item.has("deletedAt")) {
                    continue;
                }
                final String callsign = item.optString("callsign", "").trim();
                final long updatedAt = item.optLong("updatedAt", 0L);
                if (!callsign.isEmpty() && updatedAt > 0L) {
                    seenEamKeys.add(callsign.toLowerCase(Locale.US) + ":" + updatedAt);
                }
            }
        } catch (JSONException ignored) {
        }
    }

    private void maybePrimeEventKeys() {
        seenEventKeys.clear();
        try {
            final JSONObject root = new JSONObject(nonEmptyJson(ReticulumBridge.getEventsJson(), "{\"items\":[]}"));
            final JSONArray items = root.optJSONArray("items");
            if (items == null) {
                return;
            }
            for (int index = 0; index < items.length(); index += 1) {
                final JSONObject item = items.optJSONObject(index);
                if (item == null || item.has("deletedAt") || item.has("deleted_at")) {
                    continue;
                }
                final JSONObject args = item.optJSONObject("args");
                final String uid = item.optString(
                    "uid",
                    args == null ? "" : args.optString("entry_uid", "")
                ).trim();
                final long updatedAt = item.optLong("updatedAt", 0L);
                if (!uid.isEmpty() && updatedAt > 0L) {
                    seenEventKeys.add(uid.toLowerCase(Locale.US) + ":" + updatedAt);
                }
            }
        } catch (JSONException ignored) {
        }
    }

    private void maybePrimeChecklistKeys() {
        seenChecklistKeys.clear();
        try {
            final JSONObject root = new JSONObject(nonEmptyJson(
                ReticulumBridge.getChecklistsJson("{\"sortBy\":\"updated_at_desc\"}"),
                "{\"items\":[]}"
            ));
            final JSONArray items = root.optJSONArray("items");
            if (items == null) {
                return;
            }
            for (int index = 0; index < items.length(); index += 1) {
                final JSONObject item = items.optJSONObject(index);
                if (item == null || item.has("deletedAt") || item.has("deleted_at")) {
                    continue;
                }
                final String key = checklistNotificationKey(item);
                if (!key.isEmpty()) {
                    seenChecklistKeys.add(key);
                }
            }
        } catch (JSONException ignored) {
        }
    }

    private String checklistNotificationKey(JSONObject item) {
        final String uid = item.optString("uid", "").trim();
        final String stamp = latestChecklistStamp(item);
        return uid.isEmpty() || stamp.isEmpty()
            ? ""
            : uid.toLowerCase(Locale.US) + ":" + stamp;
    }

    private String latestChecklistStamp(JSONObject item) {
        String latest = "";
        for (String key : new String[] {"updatedAt", "updated_at", "uploadedAt", "uploaded_at"}) {
            final String value = item.optString(key, "").trim();
            if (!value.isEmpty() && value.compareTo(latest) > 0) {
                latest = value;
            }
        }
        return latest;
    }

    private String optStringAny(JSONObject item, String camelKey, String snakeKey) {
        if (item == null) {
            return "";
        }
        return item.optString(camelKey, item.optString(snakeKey, ""));
    }

    private int optIntAny(JSONObject item, String camelKey, String snakeKey, int fallback) {
        if (item == null) {
            return fallback;
        }
        return item.optInt(camelKey, item.optInt(snakeKey, fallback));
    }

    private ResolvedConfig resolveConfig(String rawConfigJson) throws JSONException {
        final JSONObject config = rawConfigJson == null || rawConfigJson.trim().isEmpty()
            ? new JSONObject()
            : new JSONObject(rawConfigJson);
        final File resolvedStorageDir = resolveStorageDir(config.optString("storageDir", ""));
        config.put("storageDir", resolvedStorageDir.getAbsolutePath());
        return new ResolvedConfig(
            config.toString(),
            canonicalize(config),
            resolvedStorageDir.getAbsolutePath()
        );
    }

    private File resolveStorageDir(String rawStorageDir) {
        final String normalized = rawStorageDir == null ? "" : rawStorageDir.trim();
        final File filesDir = getFilesDir();
        if (normalized.isEmpty()) {
            return new File(filesDir, "reticulum-mobile");
        }

        final File candidate = new File(normalized);
        return candidate.isAbsolute() ? candidate : new File(filesDir, normalized);
    }

    private int currentBootCount() {
        try {
            return Settings.Global.getInt(getContentResolver(), Settings.Global.BOOT_COUNT);
        } catch (Settings.SettingNotFoundException ex) {
            return 0;
        }
    }

    private int nextNotificationId() {
        final int current = nextBackgroundNotificationId;
        nextBackgroundNotificationId += 1;
        if (nextBackgroundNotificationId > BACKGROUND_NOTIFICATION_BASE_ID + 10_000) {
            nextBackgroundNotificationId = BACKGROUND_NOTIFICATION_BASE_ID;
        }
        return current;
    }

    private String truncate(String value) {
        if (value == null) {
            return "";
        }
        final String normalized = value.trim();
        if (normalized.length() <= 160) {
            return normalized;
        }
        return normalized.substring(0, 157) + "...";
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

    private String primaryAndroidAbi() {
        if (Build.SUPPORTED_ABIS == null || Build.SUPPORTED_ABIS.length == 0) {
            return "";
        }
        return Build.SUPPORTED_ABIS[0];
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

    private String canonicalize(Object value) throws JSONException {
        if (value == null || value == JSONObject.NULL) {
            return "null";
        }
        if (value instanceof JSONObject) {
            final JSONObject object = (JSONObject) value;
            final List<String> keys = new ArrayList<>();
            final Iterator<String> iterator = object.keys();
            while (iterator.hasNext()) {
                keys.add(iterator.next());
            }
            Collections.sort(keys);
            final StringBuilder builder = new StringBuilder();
            builder.append("{");
            for (int index = 0; index < keys.size(); index += 1) {
                final String key = keys.get(index);
                if (index > 0) {
                    builder.append(",");
                }
                builder.append(JSONObject.quote(key));
                builder.append(":");
                builder.append(canonicalize(object.opt(key)));
            }
            builder.append("}");
            return builder.toString();
        }
        if (value instanceof JSONArray) {
            final JSONArray array = (JSONArray) value;
            final StringBuilder builder = new StringBuilder();
            builder.append("[");
            for (int index = 0; index < array.length(); index += 1) {
                if (index > 0) {
                    builder.append(",");
                }
                builder.append(canonicalize(array.opt(index)));
            }
            builder.append("]");
            return builder.toString();
        }
        if (value instanceof String) {
            return JSONObject.quote((String) value);
        }
        if (value instanceof Number || value instanceof Boolean) {
            return String.valueOf(value);
        }
        return JSONObject.quote(String.valueOf(value));
    }

    private static final class ResolvedConfig {
        final String resolvedJson;
        final String canonicalConfig;
        final String storageDir;

        ResolvedConfig(String resolvedJson, String canonicalConfig, String storageDir) {
            this.resolvedJson = resolvedJson;
            this.canonicalConfig = canonicalConfig;
            this.storageDir = storageDir;
        }
    }
}
