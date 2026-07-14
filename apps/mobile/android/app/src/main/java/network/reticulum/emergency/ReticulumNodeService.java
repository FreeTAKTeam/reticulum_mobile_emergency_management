package network.reticulum.emergency;

import android.Manifest;
import android.app.Notification;
import android.app.PendingIntent;
import android.app.Service;
import android.app.Activity;
import android.app.Application;
import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothDevice;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.os.Binder;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.os.SystemClock;
import android.provider.Settings;
import android.util.Log;

import androidx.core.app.NotificationCompat;
import androidx.core.app.NotificationManagerCompat;

import com.getcapacitor.JSObject;
import com.getcapacitor.Logger;

import network.reticulum.emergency.plugins.PluginCoordinator;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.io.File;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Iterator;
import java.util.List;
import java.util.Locale;
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
    private static final String PREF_WATCH_STATUS_SERVER_ENABLED = "watchStatusServerEnabled";
    private static final String PREF_WATCH_STATUS_SERVER_PORT = "watchStatusServerPort";
    static final String ACTION_RESTORE_AFTER_BOOT = "network.reticulum.emergency.action.RESTORE_AFTER_BOOT";
    private static final String ACTION_STOP_SERVICE = "network.reticulum.emergency.action.STOP_NODE";
    private static final int FOREGROUND_NOTIFICATION_ID = 41001;
    private static final long RUNTIME_RESTORE_TIMEOUT_MS = 15_000L;
    private static final long FOREGROUND_NOTIFICATION_MIN_UPDATE_MS = 5_000L;

    private final IBinder binder = new LocalBinder();
    private final CopyOnWriteArraySet<ServiceEventListener> listeners = new CopyOnWriteArraySet<>();
    private final AtomicBoolean appUiForeground = new AtomicBoolean(false);
    private final AtomicBoolean pollerRunning = new AtomicBoolean(false);
    private final AtomicBoolean restoreRunning = new AtomicBoolean(false);
    private final ExecutorService pollerExecutor = Executors.newSingleThreadExecutor();
    private final ExecutorService restoreExecutor = Executors.newSingleThreadExecutor();
    private final Handler mainHandler = new Handler(Looper.getMainLooper());
    private final Application.ActivityLifecycleCallbacks activityLifecycleCallbacks =
        new Application.ActivityLifecycleCallbacks() {
            @Override
            public void onActivityCreated(Activity activity, Bundle savedInstanceState) {
            }

            @Override
            public void onActivityStarted(Activity activity) {
            }

            @Override
            public void onActivityResumed(Activity activity) {
                if (activity instanceof MainActivity) {
                    setAppUiForeground(true);
                }
            }

            @Override
            public void onActivityPaused(Activity activity) {
                if (activity instanceof MainActivity) {
                    setAppUiForeground(false);
                }
            }

            @Override
            public void onActivityStopped(Activity activity) {
                if (activity instanceof MainActivity) {
                    setAppUiForeground(false);
                }
            }

            @Override
            public void onActivitySaveInstanceState(Activity activity, Bundle outState) {
            }

            @Override
            public void onActivityDestroyed(Activity activity) {
                if (activity instanceof MainActivity) {
                    setAppUiForeground(false);
                }
            }
        };

    private SharedPreferences preferences;
    private String storageDir = "";
    private String lastResolvedConfigJson = "";
    private String lastCanonicalConfigJson = "";
    private String latestStatusJson = "";
    private String latestSyncStatusJson = "";
    private String latestSosStatusJson = "";
    private String latestRuntimeErrorJson = "";
    private long lastForegroundNotificationUpdateMs = 0L;
    private String lastForegroundNotificationFingerprint = "";
    private SosPlatformCoordinator sosPlatformCoordinator;
    private RemWatchStatusServer watchStatusServer;
    private PluginCoordinator pluginCoordinator;
    private ServiceNotificationController notificationController;

    @Override
    public void onCreate() {
        super.onCreate();
        preferences = getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE);
        storageDir = resolveStorageDir("").getAbsolutePath();
        initializeBridgeStorage(storageDir);
        notificationController = new ServiceNotificationController(
            this,
            mainHandler,
            restoreExecutor,
            this::isAppUiForeground,
            () -> latestStatusJson
        );
        notificationController.createChannels();
        sosPlatformCoordinator = new SosPlatformCoordinator(this);
        watchStatusServer = new RemWatchStatusServer();
        pluginCoordinator = new PluginCoordinator(this);
        pluginCoordinator.refresh();
        getApplication().registerActivityLifecycleCallbacks(activityLifecycleCallbacks);
        latestStatusJson = safeStatusJson();
        latestSyncStatusJson = safeSyncStatusJson();
        latestSosStatusJson = safeSosStatusJson();
        applyCurrentSosPlatformSettings();
        applyWatchStatusServerSettings();
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent != null && ACTION_STOP_SERVICE.equals(intent.getAction())) {
            stopNode();
            return START_NOT_STICKY;
        }
        if (intent != null && ACTION_RESTORE_AFTER_BOOT.equals(intent.getAction())) {
            scheduleRuntimeRestore("boot");
            return START_STICKY;
        }

        if (shouldBeRunning()) {
            scheduleRuntimeRestore("process recreation");
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
        if (watchStatusServer != null) {
            watchStatusServer.stop();
        }
        if (sosPlatformCoordinator != null) {
            sosPlatformCoordinator.close();
        }
        if (pluginCoordinator != null) {
            pluginCoordinator.close();
        }
        getApplication().unregisterActivityLifecycleCallbacks(activityLifecycleCallbacks);
        restoreExecutor.shutdownNow();
        pollerExecutor.shutdownNow();
        super.onDestroy();
    }

    @Override
    public void onTaskRemoved(Intent rootIntent) {
        super.onTaskRemoved(rootIntent);
    }

    @Override
    public void onTimeout(int startId) {
        handleForegroundServiceTimeout(startId, 0);
    }

    @Override
    public void onTimeout(int startId, int foregroundServiceType) {
        handleForegroundServiceTimeout(startId, foregroundServiceType);
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

    public void setAppUiForeground(boolean foreground) {
        if (appUiForeground.getAndSet(foreground) != foreground) {
            Log.i(TAG, "appUiForeground=" + foreground);
        }
    }

    public boolean isAppUiForeground() {
        return appUiForeground.get();
    }

    public int startNode(String configJson) {
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
                    pluginCoordinator.setNodeRunning(true);
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
                    cleanupFailedRuntimeStart();
                    return startResult;
                }
            }

            lastResolvedConfigJson = resolved.resolvedJson;
            lastCanonicalConfigJson = resolved.canonicalConfig;
            persistDesiredRunning(true, resolved);
            clearRuntimeReadinessFailure();
            notificationController.primeOperationalState();
            refreshLatestRuntimeState();
            ensurePoller();
            startForeground(FOREGROUND_NOTIFICATION_ID, buildRuntimeNotification(true));
            emitCachedStateToAll();
            emitProjectionRefreshSweepToAll();
            pluginCoordinator.setNodeRunning(true);
            return 0;
        } catch (Exception ex) {
            Logger.error(TAG, "Failed to start node", ex);
            reportRuntimeReadinessFailure(
                "InternalError",
                "node runtime failed during start: " + ex.getMessage()
            );
            cleanupFailedRuntimeStart();
            return -1;
        }
    }

    public int stopNode() {
        stopPoller();
        pluginCoordinator.setNodeRunning(false);
        final int result = ReticulumBridge.stop();
        clearDesiredRunning();
        clearRuntimeReadinessFailure();
        refreshLatestRuntimeState();
        emitCachedStateToAll();
        stopForeground(STOP_FOREGROUND_REMOVE);
        stopSelf();
        return result;
    }

    public int restartNode(String configJson) {
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
            clearRuntimeReadinessFailure();
            notificationController.primeOperationalState();
            refreshLatestRuntimeState();
            ensurePoller();
            startForeground(FOREGROUND_NOTIFICATION_ID, buildRuntimeNotification(true));
            emitCachedStateToAll();
            emitProjectionRefreshSweepToAll();
            pluginCoordinator.setNodeRunning(true);
            return 0;
        } catch (Exception ex) {
            Logger.error(TAG, "Failed to restart node", ex);
            reportRuntimeReadinessFailure(
                "InternalError",
                "node runtime failed during restart: " + ex.getMessage()
            );
            return -1;
        }
    }

    public String getStatusJson() {
        return ReticulumBridge.getStatusJson();
    }

    public int connectPeer(String destinationHex) {
        return ReticulumBridge.connectPeer(destinationHex);
    }

    public int disconnectPeer(String destinationHex) {
        return ReticulumBridge.disconnectPeer(destinationHex);
    }

    public int announceNow() {
        return ReticulumBridge.announceNow();
    }

    public int requestPeerIdentity(String destinationHex) {
        return ReticulumBridge.requestPeerIdentity(destinationHex);
    }

    public int sendJson(String payloadJson) {
        return ReticulumBridge.sendJson(payloadJson);
    }

    public String sendLxmfJson(String payloadJson) {
        return ReticulumBridge.sendLxmfJson(payloadJson);
    }

    public int retryLxmfJson(String payloadJson) {
        return ReticulumBridge.retryLxmfJson(payloadJson);
    }

    public int cancelLxmfJson(String payloadJson) {
        return ReticulumBridge.cancelLxmfJson(payloadJson);
    }

    public int broadcastBase64(String bytesBase64) {
        return ReticulumBridge.broadcastBase64(bytesBase64);
    }

    public int setActivePropagationNodeJson(String payloadJson) {
        return ReticulumBridge.setActivePropagationNodeJson(payloadJson);
    }

    public int requestLxmfSyncJson(String payloadJson) {
        return ReticulumBridge.requestLxmfSyncJson(payloadJson);
    }

    public String listAnnouncesJson() {
        return ReticulumBridge.listAnnouncesJson();
    }

    public String listPeersJson() {
        return ReticulumBridge.listPeersJson();
    }

    public String listConversationsJson() {
        return ReticulumBridge.listConversationsJson();
    }

    public String listMessagesJson(String payloadJson) {
        return ReticulumBridge.listMessagesJson(payloadJson);
    }

    public int deleteConversationJson(String payloadJson) {
        return ReticulumBridge.deleteConversationJson(payloadJson);
    }

    public String getLxmfSyncStatusJson() {
        return ReticulumBridge.getLxmfSyncStatusJson();
    }

    public String listTelemetryDestinationsJson() {
        return ReticulumBridge.listTelemetryDestinationsJson();
    }

    public String refreshPluginsJson() {
        return pluginCoordinator.refresh();
    }

    public String listPluginsJson() {
        return pluginCoordinator.listPlugins();
    }

    public int approvePluginPublisherJson(String payloadJson) {
        final int result = ReticulumBridge.approvePluginPublisherJson(payloadJson);
        if (result == 0) {
            pluginCoordinator.reconcileNow();
        }
        return result;
    }

    public int revokePluginPublisherJson(String payloadJson) {
        final int result = ReticulumBridge.revokePluginPublisherJson(payloadJson);
        if (result == 0) {
            pluginCoordinator.reconcileNow();
        }
        return result;
    }

    public int setPluginEnabledJson(String payloadJson) {
        final int result = ReticulumBridge.setPluginEnabledJson(payloadJson);
        if (result == 0) {
            pluginCoordinator.reconcileNow();
        }
        return result;
    }

    public int grantPluginCapabilitiesJson(String payloadJson) {
        final int result = ReticulumBridge.grantPluginCapabilitiesJson(payloadJson);
        if (result == 0) {
            pluginCoordinator.reconcileNow();
        }
        return result;
    }

    public String listPluginSensorsJson() {
        return pluginCoordinator.listSensors();
    }

    public Intent pluginConfigurationIntent(String pluginId) {
        return pluginCoordinator.configurationIntent(pluginId);
    }

    public String legacyImportCompletedJson() {
        return ReticulumBridge.legacyImportCompletedJson();
    }

    public int importLegacyStateJson(String payloadJson) {
        return ReticulumBridge.importLegacyStateJson(payloadJson);
    }

    public String getAppSettingsJson() {
        return ReticulumBridge.getAppSettingsJson();
    }

    public int setAppSettingsJson(String payloadJson) {
        return ReticulumBridge.setAppSettingsJson(payloadJson);
    }

    public String getWatchStatusServerSettingsJson() {
        try {
            return currentWatchStatusServerStateJson().toString();
        } catch (JSONException ex) {
            return "{}";
        }
    }

    public int setWatchStatusServerSettingsJson(String payloadJson) {
        try {
            final JSONObject payload = new JSONObject(nonEmptyJson(payloadJson, "{}"));
            final RemWatchStatusServerSettings current = readWatchStatusServerSettings();
            final boolean enabled = payload.has("enabled") ? payload.optBoolean("enabled", current.enabled) : current.enabled;
            final int requestedPort = payload.has("port") ? payload.optInt("port", current.port) : current.port;
            final RemWatchStatusServerSettings next = RemWatchStatusServerSettings.normalize(enabled, requestedPort);
            preferences
                .edit()
                .putBoolean(PREF_WATCH_STATUS_SERVER_ENABLED, next.enabled)
                .putInt(PREF_WATCH_STATUS_SERVER_PORT, next.port)
                .apply();
            applyWatchStatusServerSettings();
            return 0;
        } catch (JSONException ex) {
            Logger.error(TAG, "Failed to parse watch status server settings", ex);
            return -1;
        }
    }

    public String getWatchStatusServerStateJson() {
        try {
            return currentWatchStatusServerStateJson().toString();
        } catch (JSONException ex) {
            return "{}";
        }
    }

    public String getSavedPeersJson() {
        return ReticulumBridge.getSavedPeersJson();
    }

    public int setSavedPeersJson(String payloadJson) {
        return ReticulumBridge.setSavedPeersJson(payloadJson);
    }

    public String getOperationalSummaryJson() {
        return ReticulumBridge.getOperationalSummaryJson();
    }

    public String getChecklistsJson(String payloadJson) {
        return ReticulumBridge.getChecklistsJson(payloadJson);
    }

    public String getChecklistJson(String payloadJson) {
        return ReticulumBridge.getChecklistJson(payloadJson);
    }

    public String getChecklistTemplatesJson(String payloadJson) {
        return ReticulumBridge.getChecklistTemplatesJson(payloadJson);
    }

    public String importChecklistTemplateCsvJson(String payloadJson) {
        return ReticulumBridge.importChecklistTemplateCsvJson(payloadJson);
    }

    public int createChecklistFromTemplateJson(String payloadJson) {
        return ReticulumBridge.createChecklistFromTemplateJson(payloadJson);
    }

    public int createOnlineChecklistJson(String payloadJson) {
        return ReticulumBridge.createOnlineChecklistJson(payloadJson);
    }

    public int updateChecklistJson(String payloadJson) {
        return ReticulumBridge.updateChecklistJson(payloadJson);
    }

    public int deleteChecklistJson(String payloadJson) {
        return ReticulumBridge.deleteChecklistJson(payloadJson);
    }

    public int joinChecklistJson(String payloadJson) {
        return ReticulumBridge.joinChecklistJson(payloadJson);
    }

    public int uploadChecklistJson(String payloadJson) {
        return ReticulumBridge.uploadChecklistJson(payloadJson);
    }

    public int setChecklistTaskStatusJson(String payloadJson) {
        return ReticulumBridge.setChecklistTaskStatusJson(payloadJson);
    }

    public int addChecklistTaskRowJson(String payloadJson) {
        return ReticulumBridge.addChecklistTaskRowJson(payloadJson);
    }

    public int deleteChecklistTaskRowJson(String payloadJson) {
        return ReticulumBridge.deleteChecklistTaskRowJson(payloadJson);
    }

    public int setChecklistTaskRowStyleJson(String payloadJson) {
        return ReticulumBridge.setChecklistTaskRowStyleJson(payloadJson);
    }

    public int setChecklistTaskCellJson(String payloadJson) {
        return ReticulumBridge.setChecklistTaskCellJson(payloadJson);
    }

    public String getEamsJson() {
        return ReticulumBridge.getEamsJson();
    }

    public int upsertEamJson(String payloadJson) {
        return ReticulumBridge.upsertEamJson(payloadJson);
    }

    public int deleteEamJson(String payloadJson) {
        return ReticulumBridge.deleteEamJson(payloadJson);
    }

    public int deleteLocalEamJson(String payloadJson) {
        return ReticulumBridge.deleteLocalEamJson(payloadJson);
    }

    public String getEamTeamSummaryJson(String payloadJson) {
        return ReticulumBridge.getEamTeamSummaryJson(payloadJson);
    }

    public String getEamReadinessSummaryJson() {
        return ReticulumBridge.getEamReadinessSummaryJson();
    }

    public String getEventsJson() {
        return ReticulumBridge.getEventsJson();
    }

    public int upsertEventJson(String payloadJson) {
        return ReticulumBridge.upsertEventJson(payloadJson);
    }

    public int deleteEventJson(String payloadJson) {
        return ReticulumBridge.deleteEventJson(payloadJson);
    }

    public String getTelemetryPositionsJson() {
        return ReticulumBridge.getTelemetryPositionsJson();
    }

    public int recordLocalTelemetryFixJson(String payloadJson) {
        return ReticulumBridge.recordLocalTelemetryFixJson(payloadJson);
    }

    public int deleteLocalTelemetryJson(String payloadJson) {
        return ReticulumBridge.deleteLocalTelemetryJson(payloadJson);
    }

    public String getSosSettingsJson() {
        return ReticulumBridge.getSosSettingsJson();
    }

    public int setSosSettingsJson(String payloadJson) {
        final int result = ReticulumBridge.setSosSettingsJson(payloadJson);
        if (result == 0) {
            applyCurrentSosPlatformSettings();
        }
        return result;
    }

    public int setSosPinJson(String payloadJson) {
        return ReticulumBridge.setSosPinJson(payloadJson);
    }

    public String getSosStatusJson() {
        return ReticulumBridge.getSosStatusJson();
    }

    public String triggerSosJson(String payloadJson) {
        if (sosPlatformCoordinator != null) {
            sosPlatformCoordinator.submitTelemetrySnapshot();
        }
        return ReticulumBridge.triggerSosJson(payloadJson);
    }

    public String deactivateSosJson(String payloadJson) {
        return ReticulumBridge.deactivateSosJson(payloadJson);
    }

    public int submitSosTelemetryJson(String payloadJson) {
        return ReticulumBridge.submitSosTelemetryJson(payloadJson);
    }

    public String submitSosAccelerometerJson(String payloadJson) {
        return ReticulumBridge.submitSosAccelerometerJson(payloadJson);
    }

    public String submitSosScreenEventJson(String payloadJson) {
        return ReticulumBridge.submitSosScreenEventJson(payloadJson);
    }

    public String listSosAlertsJson() {
        return ReticulumBridge.listSosAlertsJson();
    }

    public String listSosLocationsJson() {
        return ReticulumBridge.listSosLocationsJson();
    }

    public String listSosAudioJson() {
        return ReticulumBridge.listSosAudioJson();
    }

    public int recordSosAudioJson(String payloadJson) {
        return ReticulumBridge.recordSosAudioJson(payloadJson);
    }

    public int setAnnounceCapabilities(String capabilityString) {
        return ReticulumBridge.setAnnounceCapabilities(capabilityString);
    }

    public int setLogLevel(String levelString) {
        return ReticulumBridge.setLogLevel(levelString);
    }

    public int refreshHubDirectory() {
        return ReticulumBridge.refreshHubDirectory();
    }

    public String takeLastErrorJson() {
        return ReticulumBridge.takeLastErrorJson();
    }

    private void scheduleRuntimeRestore(String reason) {
        if (!shouldBeRunning()) {
            return;
        }

        final String persistedConfig = preferences.getString(PREF_LAST_CONFIG, "");
        if (persistedConfig == null || persistedConfig.trim().isEmpty()) {
            return;
        }

        if (!restoreRunning.compareAndSet(false, true)) {
            return;
        }

        promoteServiceForRuntime();
        final AtomicBoolean restoreCompleted = new AtomicBoolean(false);
        mainHandler.postDelayed(() -> {
            if (restoreCompleted.get() || !restoreRunning.get()) {
                return;
            }
            reportRuntimeReadinessFailure(
                "InternalError",
                "node runtime restore timed out after " + RUNTIME_RESTORE_TIMEOUT_MS + "ms"
            );
        }, RUNTIME_RESTORE_TIMEOUT_MS);
        restoreExecutor.execute(() -> {
            try {
                if (isNodeRunning()) {
                    ensurePoller();
                    clearRuntimeReadinessFailure();
                    refreshLatestRuntimeState();
                    startForeground(FOREGROUND_NOTIFICATION_ID, buildRuntimeNotification(true));
                    emitCachedStateToAll();
                    emitProjectionRefreshSweepToAll();
                    return;
                }

                final int result = startNode(persistedConfig);
                if (result != 0) {
                    reportRuntimeReadinessFailure(
                        "InternalError",
                        "node runtime failed to restore after " + reason
                    );
                }
            } catch (Exception ex) {
                Logger.error(TAG, "Failed to restore node after " + reason, ex);
                reportRuntimeReadinessFailure(
                    "InternalError",
                    "node runtime failed to restore after " + reason + ": " + ex.getMessage()
                );
            } finally {
                restoreCompleted.set(true);
                restoreRunning.set(false);
            }
        });
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

    private void cleanupFailedRuntimeStart() {
        stopPoller();
        pluginCoordinator.setNodeRunning(false);
        try {
            ReticulumBridge.stop();
        } catch (Exception ex) {
            Log.w(TAG, "Failed to stop native runtime after start failure", ex);
        }
        clearDesiredRunning();
        refreshLatestRuntimeState();
        emitCachedStateToAll();
        stopForegroundAndSelf(0);
    }

    private synchronized void handleForegroundServiceTimeout(int startId, int foregroundServiceType) {
        reportRuntimeReadinessFailure(
            "InternalError",
            "node runtime foreground service timed out; stopping Reticulum node service. type="
                + foregroundServiceType
        );
        stopPoller();
        pluginCoordinator.setNodeRunning(false);
        try {
            ReticulumBridge.stop();
        } catch (Exception ex) {
            Log.w(TAG, "Failed to stop native runtime after foreground service timeout", ex);
        }
        clearDesiredRunning();
        refreshLatestRuntimeState();
        emitCachedStateToAll();
        stopForegroundAndSelf(startId);
    }

    private void clearRuntimeReadinessFailure() {
        latestRuntimeErrorJson = "";
    }

    private void reportRuntimeReadinessFailure(String code, String message) {
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
            statusPayload.put("status", new JSObject(nonEmptyJson(latestStatusJson, "{}")));
        } catch (JSONException ignored) {
            statusPayload.put("status", new JSObject());
        }
        dispatchEventToListeners("statusChanged", statusPayload);
        updateForegroundNotification();
    }

    private String statusJsonWithLastError(String message) {
        try {
            final JSONObject status = new JSONObject(nonEmptyJson(ReticulumBridge.getStatusJson(), "{}"));
            status.put("running", false);
            status.put("lastError", message);
            return status.toString();
        } catch (JSONException ex) {
            final JSONObject status = new JSONObject();
            try {
                status.put("running", false);
                status.put("lastError", message);
            } catch (JSONException ignored) {
                return "{\"running\":false}";
            }
            return status.toString();
        }
    }

    private void stopForegroundAndSelf(int startId) {
        try {
            stopForeground(STOP_FOREGROUND_REMOVE);
        } catch (Exception ex) {
            Log.w(TAG, "Failed to remove foreground notification", ex);
        }
        if (startId > 0) {
            stopSelf(startId);
        } else {
            stopSelf();
        }
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
        final boolean uiForeground = isAppUiForeground();
        if ("sosAlertChanged".equals(eventName)) {
            notificationController.handleSosAlert(payload, !uiForeground);
        } else if (!uiForeground) {
            notificationController.handleInboundUpdate(eventName, payload);
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

    private void dispatchEventToListeners(String eventName, JSObject payload) {
        if (!NativeEventBackpressure.shouldDispatchToUi(eventName, payload)) {
            return;
        }
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

        if (latestRuntimeErrorJson != null && !latestRuntimeErrorJson.trim().isEmpty()) {
            try {
                listener.onNodeEvent("error", new JSObject(latestRuntimeErrorJson));
            } catch (JSONException ignored) {
                final JSObject fallback = new JSObject();
                fallback.put("code", "InternalError");
                fallback.put("message", "node runtime failed");
                listener.onNodeEvent("error", fallback);
            }
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

    private RemWatchStatusServerSettings readWatchStatusServerSettings() {
        if (preferences == null) {
            return RemWatchStatusServerSettings.defaults();
        }
        return RemWatchStatusServerSettings.normalize(
            preferences.getBoolean(
                PREF_WATCH_STATUS_SERVER_ENABLED,
                RemWatchStatusServerSettings.DEFAULT_ENABLED
            ),
            preferences.getInt(
                PREF_WATCH_STATUS_SERVER_PORT,
                RemWatchStatusServerSettings.DEFAULT_PORT
            )
        );
    }

    private void applyWatchStatusServerSettings() {
        if (watchStatusServer == null) {
            return;
        }
        watchStatusServer.apply(readWatchStatusServerSettings(), this::buildWatchStatusJson);
    }

    private JSONObject currentWatchStatusServerStateJson() throws JSONException {
        if (watchStatusServer == null) {
            return readWatchStatusServerSettings().toJson(false, "");
        }
        return watchStatusServer.stateJson();
    }

    private String buildWatchStatusJson() {
        try {
            return RemWatchStatusPayload.build(
                safeStatusJson(),
                safeOperationalSummaryJson(),
                safeEamReadinessSummaryJson(),
                safeEventsJson(),
                safeTelemetryPositionsJson(),
                System.currentTimeMillis()
            );
        } catch (Exception ex) {
            Logger.error(TAG, "Failed to build watch status payload", ex);
            try {
                final JSONObject fallbackStatus = new JSONObject();
                fallbackStatus.put("running", false);
                fallbackStatus.put("lastError", ex.getMessage() == null ? ex.toString() : ex.getMessage());
                return RemWatchStatusPayload.build(
                    fallbackStatus.toString(),
                    "{}",
                    "{}",
                    "{\"items\":[]}",
                    "{\"items\":[]}",
                    System.currentTimeMillis()
                );
            } catch (JSONException jsonException) {
                return "{\"type\":\"rem.watch.status\",\"version\":1,\"connection_state\":\"ERROR\",\"operator_name\":\"REM\",\"operator_status\":\"ERROR\",\"operator_eam\":\"UNKNOWN\",\"team\":\"REM\",\"team_status\":\"UNKNOWN\",\"last_sync_epoch_ms\":0,\"last_sync_age_seconds\":0,\"active_events\":0,\"highest_priority\":\"ERROR\",\"alert_state\":\"ERROR\"}";
            }
        }
    }

    private String safeOperationalSummaryJson() {
        try {
            return nonEmptyJson(ReticulumBridge.getOperationalSummaryJson(), "{}");
        } catch (Exception ex) {
            return "{}";
        }
    }

    private String safeEamReadinessSummaryJson() {
        try {
            return nonEmptyJson(ReticulumBridge.getEamReadinessSummaryJson(), "{}");
        } catch (Exception ex) {
            return "{}";
        }
    }

    private String safeEventsJson() {
        try {
            return nonEmptyJson(ReticulumBridge.getEventsJson(), "{\"items\":[]}");
        } catch (Exception ex) {
            return "{\"items\":[]}";
        }
    }

    private String safeTelemetryPositionsJson() {
        try {
            return nonEmptyJson(ReticulumBridge.getTelemetryPositionsJson(), "{\"items\":[]}");
        } catch (Exception ex) {
            return "{\"items\":[]}";
        }
    }

    private String nonEmptyJson(String raw, String fallback) {
        if (raw == null || raw.trim().isEmpty()) {
            return fallback;
        }
        return raw;
    }

    private void promoteServiceForRuntime() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            startForeground(FOREGROUND_NOTIFICATION_ID, buildRuntimeNotification(false));
        }
    }

    private Notification buildRuntimeNotification(boolean running) {
        return buildRuntimeNotification(running, buildRuntimeNotificationBody(running));
    }

    private Notification buildRuntimeNotification(boolean running, String body) {
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
        final String safeBody = body == null || body.trim().isEmpty()
            ? getString(R.string.app_name)
            : body;

        return new NotificationCompat.Builder(this, ServiceNotificationController.RUNTIME_CHANNEL_ID)
            .setContentTitle(title)
            .setContentText(safeBody)
            .setStyle(new NotificationCompat.BigTextStyle().bigText(safeBody))
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
        final String body = buildRuntimeNotificationBody(true);
        final String fingerprint = "running|" + body;
        final long now = SystemClock.elapsedRealtime();
        synchronized (this) {
            if (fingerprint.equals(lastForegroundNotificationFingerprint)) {
                return;
            }
            final long elapsed = now - lastForegroundNotificationUpdateMs;
            if (lastForegroundNotificationUpdateMs > 0L && elapsed < FOREGROUND_NOTIFICATION_MIN_UPDATE_MS) {
                return;
            }
            lastForegroundNotificationUpdateMs = now;
            lastForegroundNotificationFingerprint = fingerprint;
        }
        NotificationManagerCompat.from(this).notify(
            FOREGROUND_NOTIFICATION_ID,
            buildRuntimeNotification(true, body)
        );
    }

    private ResolvedConfig resolveConfig(String rawConfigJson) throws JSONException {
        final JSONObject config = rawConfigJson == null || rawConfigJson.trim().isEmpty()
            ? new JSONObject()
            : new JSONObject(rawConfigJson);
        repairRnodeConfig(config);
        final File resolvedStorageDir = resolveStorageDir(config.optString("storageDir", ""));
        config.put("storageDir", resolvedStorageDir.getAbsolutePath());
        return new ResolvedConfig(
            config.toString(),
            canonicalize(config),
            resolvedStorageDir.getAbsolutePath()
        );
    }

    private void repairRnodeConfig(JSONObject config) throws JSONException {
        final JSONObject rnode = config.optJSONObject("rnode");
        if (rnode == null || !rnode.optBoolean("enabled", false)) {
            return;
        }
        if (!RNodeConnectionModes.usesBluetoothRepair(
            rnode.optString("connectionMode", rnode.optString("connection_mode", ""))
        )) {
            return;
        }
        final String configuredId = rnode.optString("peripheralId", "").trim();
        if (configuredId.isEmpty() || !hasBluetoothConnectPermission()) {
            return;
        }
        final BluetoothAdapter adapter = BluetoothAdapter.getDefaultAdapter();
        if (adapter == null || !adapter.isEnabled()) {
            return;
        }

        BluetoothDevice singleRnode = null;
        int rnodeCount = 0;
        try {
            for (BluetoothDevice device : adapter.getBondedDevices()) {
                if (deviceMatchesId(device, configuredId)) {
                    return;
                }
                if (isRnodeBluetoothDevice(device)) {
                    singleRnode = device;
                    rnodeCount += 1;
                }
            }
        } catch (SecurityException ex) {
            return;
        }

        if (rnodeCount != 1 || singleRnode == null) {
            return;
        }
        final String address = singleRnode.getAddress();
        if (address == null || address.trim().isEmpty()) {
            return;
        }
        String name = "";
        try {
            name = singleRnode.getName();
        } catch (SecurityException ignored) {
        }
        rnode.put("peripheralId", address);
        rnode.put("displayName", name == null || name.trim().isEmpty() ? address : name.trim());
        Log.i(
            TAG,
            "RNode config repaired from stale peripheral " + configuredId
                + " to bonded " + address
        );
    }

    private boolean hasBluetoothConnectPermission() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            return checkSelfPermission(Manifest.permission.BLUETOOTH_CONNECT)
                == PackageManager.PERMISSION_GRANTED;
        }
        return checkSelfPermission(Manifest.permission.BLUETOOTH)
            == PackageManager.PERMISSION_GRANTED;
    }

    private boolean isRnodeBluetoothDevice(BluetoothDevice device) {
        if (device == null || device.getBondState() != BluetoothDevice.BOND_BONDED) {
            return false;
        }
        try {
            final String name = device.getName();
            return name != null && name.toLowerCase(Locale.US).contains("rnode");
        } catch (SecurityException ex) {
            return false;
        }
    }

    private boolean deviceMatchesId(BluetoothDevice device, String configuredId) {
        if (device == null) {
            return false;
        }
        final String target = normalizeBluetoothId(configuredId);
        if (target.isEmpty()) {
            return false;
        }
        final String address = device.getAddress();
        if (normalizeBluetoothId(address).equals(target)) {
            return true;
        }
        try {
            return normalizeBluetoothId(device.getName()).equals(target);
        } catch (SecurityException ex) {
            return false;
        }
    }

    private String normalizeBluetoothId(String value) {
        if (value == null) {
            return "";
        }
        return value.trim().replace(":", "").replace("-", "").toLowerCase(Locale.US);
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
