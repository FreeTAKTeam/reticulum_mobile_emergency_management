package network.reticulum.emergency;

import android.app.Notification;
import android.app.PendingIntent;
import android.app.Activity;
import android.app.Application;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.os.Binder;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.os.SystemClock;
import android.util.Log;

import androidx.core.app.NotificationCompat;
import androidx.core.app.NotificationManagerCompat;

import com.getcapacitor.JSObject;
import com.getcapacitor.Logger;

import network.reticulum.emergency.plugins.PluginCoordinator;

import org.json.JSONException;
import org.json.JSONObject;

import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicBoolean;

public final class ReticulumNodeService extends ReticulumBridgeServiceApi {
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
    private final AtomicBoolean appUiForeground = new AtomicBoolean(false);
    private final AtomicBoolean restoreRunning = new AtomicBoolean(false);
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
    private long lastForegroundNotificationUpdateMs = 0L;
    private String lastForegroundNotificationFingerprint = "";
    private SosPlatformCoordinator sosPlatformCoordinator;
    private RemWatchStatusServer watchStatusServer;
    private PluginCoordinator pluginCoordinator;
    private ServiceNotificationController notificationController;
    private RuntimeConfigResolver configResolver;
    private ServiceEventCoordinator eventCoordinator;

    @Override
    public void onCreate() {
        super.onCreate();
        preferences = getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE);
        configResolver = new RuntimeConfigResolver(this);
        storageDir = configResolver.resolveStorageDir("").getAbsolutePath();
        initializeBridgeStorage(storageDir);
        notificationController = new ServiceNotificationController(
            this,
            mainHandler,
            restoreExecutor,
            this::isAppUiForeground,
            this::latestStatusJson
        );
        notificationController.createChannels();
        sosPlatformCoordinator = new SosPlatformCoordinator(this);
        watchStatusServer = new RemWatchStatusServer();
        pluginCoordinator = new PluginCoordinator(this);
        pluginCoordinator.refresh();
        eventCoordinator = new ServiceEventCoordinator(
            mainHandler,
            notificationController,
            pluginCoordinator,
            sosPlatformCoordinator,
            this::isAppUiForeground,
            this::updateForegroundNotification
        );
        getApplication().registerActivityLifecycleCallbacks(activityLifecycleCallbacks);
        eventCoordinator.refreshLatestRuntimeState();
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
        if (eventCoordinator != null) {
            eventCoordinator.close();
        }
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
        eventCoordinator.addListener(listener);
    }

    public void removeListener(ServiceEventListener listener) {
        eventCoordinator.removeListener(listener);
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
            final RuntimeConfigResolver.ResolvedConfig resolved = configResolver.resolve(configJson);
            initializeBridgeStorage(resolved.storageDir);
            if (isNodeRunning()) {
                if (resolved.canonicalConfig.equals(lastCanonicalConfigJson)) {
                    persistDesiredRunning(true, resolved);
                    eventCoordinator.start();
                    eventCoordinator.refreshLatestRuntimeState();
                    startForeground(FOREGROUND_NOTIFICATION_ID, buildRuntimeNotification(true));
                    eventCoordinator.emitCachedStateToAll();
                    eventCoordinator.emitProjectionRefreshSweepToAll();
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
            eventCoordinator.clearRuntimeReadinessFailure();
            notificationController.primeOperationalState();
            eventCoordinator.refreshLatestRuntimeState();
            eventCoordinator.start();
            startForeground(FOREGROUND_NOTIFICATION_ID, buildRuntimeNotification(true));
            eventCoordinator.emitCachedStateToAll();
            eventCoordinator.emitProjectionRefreshSweepToAll();
            pluginCoordinator.setNodeRunning(true);
            return 0;
        } catch (Exception ex) {
            Logger.error(TAG, "Failed to start node", ex);
            eventCoordinator.reportRuntimeReadinessFailure(
                "InternalError",
                "node runtime failed during start: " + ex.getMessage()
            );
            cleanupFailedRuntimeStart();
            return -1;
        }
    }

    public int stopNode() {
        eventCoordinator.stop();
        pluginCoordinator.setNodeRunning(false);
        final int result = ReticulumBridge.stop();
        clearDesiredRunning();
        eventCoordinator.clearRuntimeReadinessFailure();
        eventCoordinator.refreshLatestRuntimeState();
        eventCoordinator.emitCachedStateToAll();
        stopForeground(STOP_FOREGROUND_REMOVE);
        stopSelf();
        return result;
    }

    public int restartNode(String configJson) {
        try {
            final RuntimeConfigResolver.ResolvedConfig resolved = configResolver.resolve(configJson);
            promoteServiceForRuntime();
            final int result = ReticulumBridge.restart(resolved.resolvedJson);
            if (result != 0) {
                return result;
            }

            lastResolvedConfigJson = resolved.resolvedJson;
            lastCanonicalConfigJson = resolved.canonicalConfig;
            persistDesiredRunning(true, resolved);
            eventCoordinator.clearRuntimeReadinessFailure();
            notificationController.primeOperationalState();
            eventCoordinator.refreshLatestRuntimeState();
            eventCoordinator.start();
            startForeground(FOREGROUND_NOTIFICATION_ID, buildRuntimeNotification(true));
            eventCoordinator.emitCachedStateToAll();
            eventCoordinator.emitProjectionRefreshSweepToAll();
            pluginCoordinator.setNodeRunning(true);
            return 0;
        } catch (Exception ex) {
            Logger.error(TAG, "Failed to restart node", ex);
            eventCoordinator.reportRuntimeReadinessFailure(
                "InternalError",
                "node runtime failed during restart: " + ex.getMessage()
            );
            return -1;
        }
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

    public String getWatchStatusServerSettingsJson() {
        try {
            return currentWatchStatusServerStateJson().toString();
        } catch (JSONException ex) {
            return "{}";
        }
    }

    public int setWatchStatusServerSettingsJson(String payloadJson) {
        try {
            final JSONObject payload = new JSONObject(JsonPayloads.orFallback(payloadJson, "{}"));
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

    public int setSosSettingsJson(String payloadJson) {
        final int result = ReticulumBridge.setSosSettingsJson(payloadJson);
        if (result == 0) {
            eventCoordinator.applyCurrentSosPlatformSettings();
        }
        return result;
    }

    public String triggerSosJson(String payloadJson) {
        if (sosPlatformCoordinator != null) {
            sosPlatformCoordinator.submitTelemetrySnapshot();
        }
        return ReticulumBridge.triggerSosJson(payloadJson);
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
            eventCoordinator.reportRuntimeReadinessFailure(
                "InternalError",
                "node runtime restore timed out after " + RUNTIME_RESTORE_TIMEOUT_MS + "ms"
            );
        }, RUNTIME_RESTORE_TIMEOUT_MS);
        restoreExecutor.execute(() -> {
            try {
                if (isNodeRunning()) {
                    eventCoordinator.start();
                    eventCoordinator.clearRuntimeReadinessFailure();
                    eventCoordinator.refreshLatestRuntimeState();
                    startForeground(FOREGROUND_NOTIFICATION_ID, buildRuntimeNotification(true));
                    eventCoordinator.emitCachedStateToAll();
                    eventCoordinator.emitProjectionRefreshSweepToAll();
                    return;
                }

                final int result = startNode(persistedConfig);
                if (result != 0) {
                    eventCoordinator.reportRuntimeReadinessFailure(
                        "InternalError",
                        "node runtime failed to restore after " + reason
                    );
                }
            } catch (Exception ex) {
                Logger.error(TAG, "Failed to restore node after " + reason, ex);
                eventCoordinator.reportRuntimeReadinessFailure(
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
        return preferences.getInt(PREF_LAST_BOOT_COUNT, -1) == configResolver.currentBootCount();
    }

    private boolean isNodeRunning() {
        try {
            final JSONObject payload = new JSONObject(
                JsonPayloads.orFallback(ReticulumBridge.getStatusJson(), "{}")
            );
            return payload.optBoolean("running", false);
        } catch (JSONException ex) {
            return false;
        }
    }

    private void persistDesiredRunning(
        boolean desiredRunning,
        RuntimeConfigResolver.ResolvedConfig resolved
    ) {
        preferences.edit()
            .putBoolean(PREF_DESIRED_RUNNING, desiredRunning)
            .putString(PREF_LAST_CONFIG, resolved.resolvedJson)
            .putInt(PREF_LAST_BOOT_COUNT, configResolver.currentBootCount())
            .apply();
    }

    private void clearDesiredRunning() {
        preferences.edit()
            .putBoolean(PREF_DESIRED_RUNNING, false)
            .remove(PREF_LAST_CONFIG)
            .putInt(PREF_LAST_BOOT_COUNT, configResolver.currentBootCount())
            .apply();
        lastResolvedConfigJson = "";
        lastCanonicalConfigJson = "";
    }

    private void cleanupFailedRuntimeStart() {
        eventCoordinator.stop();
        pluginCoordinator.setNodeRunning(false);
        try {
            ReticulumBridge.stop();
        } catch (Exception ex) {
            Log.w(TAG, "Failed to stop native runtime after start failure", ex);
        }
        clearDesiredRunning();
        eventCoordinator.refreshLatestRuntimeState();
        eventCoordinator.emitCachedStateToAll();
        stopForegroundAndSelf(0);
    }

    private synchronized void handleForegroundServiceTimeout(int startId, int foregroundServiceType) {
        eventCoordinator.reportRuntimeReadinessFailure(
            "InternalError",
            "node runtime foreground service timed out; stopping Reticulum node service. type="
                + foregroundServiceType
        );
        eventCoordinator.stop();
        pluginCoordinator.setNodeRunning(false);
        try {
            ReticulumBridge.stop();
        } catch (Exception ex) {
            Log.w(TAG, "Failed to stop native runtime after foreground service timeout", ex);
        }
        clearDesiredRunning();
        eventCoordinator.refreshLatestRuntimeState();
        eventCoordinator.emitCachedStateToAll();
        stopForegroundAndSelf(startId);
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
                "Failed to initialize bridge storage: "
                    + JsonPayloads.orFallback(ReticulumBridge.takeLastErrorJson(), "unknown"),
                null
            );
        }
    }

    private String safeStatusJson() {
        return JsonPayloads.orFallback(ReticulumBridge.getStatusJson(), "{}");
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
            return JsonPayloads.orFallback(ReticulumBridge.getOperationalSummaryJson(), "{}");
        } catch (Exception ex) {
            return "{}";
        }
    }

    private String safeEamReadinessSummaryJson() {
        try {
            return JsonPayloads.orFallback(ReticulumBridge.getEamReadinessSummaryJson(), "{}");
        } catch (Exception ex) {
            return "{}";
        }
    }

    private String safeEventsJson() {
        try {
            return JsonPayloads.orFallback(ReticulumBridge.getEventsJson(), "{\"items\":[]}");
        } catch (Exception ex) {
            return "{\"items\":[]}";
        }
    }

    private String safeTelemetryPositionsJson() {
        try {
            return JsonPayloads.orFallback(
                ReticulumBridge.getTelemetryPositionsJson(),
                "{\"items\":[]}"
            );
        } catch (Exception ex) {
            return "{\"items\":[]}";
        }
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
            final JSONObject status = new JSONObject(
                JsonPayloads.orFallback(latestStatusJson(), "{}")
            );
            final JSONObject sync = new JSONObject(
                JsonPayloads.orFallback(latestSyncStatusJson(), "{}")
            );
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

    private String latestStatusJson() {
        return eventCoordinator == null ? "{}" : eventCoordinator.latestStatusJson();
    }

    private String latestSyncStatusJson() {
        return eventCoordinator == null ? "{}" : eventCoordinator.latestSyncStatusJson();
    }
}
