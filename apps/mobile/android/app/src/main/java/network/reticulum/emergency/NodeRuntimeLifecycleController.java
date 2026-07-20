package network.reticulum.emergency;

import android.app.Notification;
import android.app.PendingIntent;
import android.content.Intent;
import android.content.SharedPreferences;
import android.os.Build;
import android.os.Handler;
import android.os.SystemClock;
import android.util.Log;

import androidx.core.app.NotificationCompat;
import androidx.core.app.NotificationManagerCompat;

import com.getcapacitor.Logger;

import network.reticulum.emergency.plugins.PluginCoordinator;

import org.json.JSONException;
import org.json.JSONObject;

import java.util.concurrent.ExecutorService;
import java.util.concurrent.atomic.AtomicBoolean;

final class NodeRuntimeLifecycleController {
    private static final String TAG = "ReticulumNodeService";
    private static final String PREF_DESIRED_RUNNING = "desiredRunning";
    private static final String PREF_LAST_CONFIG = "lastConfig";
    private static final String PREF_LAST_BOOT_COUNT = "lastBootCount";
    private static final int FOREGROUND_NOTIFICATION_ID = 41001;
    private static final long RUNTIME_RESTORE_TIMEOUT_MS = 15_000L;
    private static final long FOREGROUND_NOTIFICATION_MIN_UPDATE_MS = 5_000L;

    private final ReticulumNodeService service;
    private final SharedPreferences preferences;
    private final Handler mainHandler;
    private final ExecutorService restoreExecutor;
    private final ServiceNotificationController notificationController;
    private final PluginCoordinator pluginCoordinator;
    private final RuntimeConfigResolver configResolver;
    private final RuntimeOperationGate runtimeOperationGate = new RuntimeOperationGate();
    private final AtomicBoolean restoreRunning = new AtomicBoolean(false);

    private ServiceEventCoordinator eventCoordinator;
    private String lastCanonicalConfigJson = "";
    private long lastForegroundNotificationUpdateMs = 0L;
    private String lastForegroundNotificationFingerprint = "";

    NodeRuntimeLifecycleController(
        ReticulumNodeService service,
        SharedPreferences preferences,
        Handler mainHandler,
        ExecutorService restoreExecutor,
        ServiceNotificationController notificationController,
        PluginCoordinator pluginCoordinator,
        RuntimeConfigResolver configResolver
    ) {
        this.service = service;
        this.preferences = preferences;
        this.mainHandler = mainHandler;
        this.restoreExecutor = restoreExecutor;
        this.notificationController = notificationController;
        this.pluginCoordinator = pluginCoordinator;
        this.configResolver = configResolver;
    }

    void attachEventCoordinator(ServiceEventCoordinator eventCoordinator) {
        this.eventCoordinator = eventCoordinator;
    }

    static void initializeStorage(String storageDir) {
        final int result = ReticulumBridge.initializeStorage(storageDir);
        if (result != 0) {
            Logger.error(
                TAG,
                "Failed to initialize bridge storage: "
                    + JsonPayloads.orFallback(ReticulumBridge.takeLastErrorJson(), "unknown"),
                null
            );
        }
    }

    int startNode(String configJson) {
        return runtimeOperationGate.runExplicit(() -> startNodeInternal(configJson));
    }

    private int startNodeInternal(String configJson) {
        try {
            final RuntimeConfigResolver.ResolvedConfig resolved = configResolver.resolve(configJson);
            initializeStorage(resolved.storageDir);
            if (isNodeRunning()) {
                if (resolved.canonicalConfig.equals(lastCanonicalConfigJson)) {
                    persistDesiredRunning(resolved);
                    resumeUnchangedRuntime();
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

            lastCanonicalConfigJson = resolved.canonicalConfig;
            persistDesiredRunning(resolved);
            completeRuntimeStart();
            return 0;
        } catch (Exception ex) {
            Logger.error(TAG, "Failed to start node", ex);
            events().reportRuntimeReadinessFailure(
                "InternalError",
                "node runtime failed during start: " + ex.getMessage()
            );
            cleanupFailedRuntimeStart();
            return -1;
        }
    }

    int stopNode() {
        return runtimeOperationGate.runExplicit(this::stopNodeInternal);
    }

    private int stopNodeInternal() {
        events().stop();
        pluginCoordinator.setNodeRunning(false);
        final int result = ReticulumBridge.stop();
        clearDesiredRunning();
        events().clearRuntimeReadinessFailure();
        events().refreshLatestRuntimeState();
        events().emitCachedStateToAll();
        service.stopForeground(ReticulumNodeService.STOP_FOREGROUND_REMOVE);
        service.stopSelf();
        return result;
    }

    int restartNode(String configJson) {
        return runtimeOperationGate.runExplicit(() -> restartNodeInternal(configJson));
    }

    private int restartNodeInternal(String configJson) {
        try {
            final RuntimeConfigResolver.ResolvedConfig resolved = configResolver.resolve(configJson);
            promoteServiceForRuntime();
            final int result = ReticulumBridge.restart(resolved.resolvedJson);
            if (result != 0) {
                return result;
            }

            lastCanonicalConfigJson = resolved.canonicalConfig;
            persistDesiredRunning(resolved);
            completeRuntimeStart();
            return 0;
        } catch (Exception ex) {
            Logger.error(TAG, "Failed to restart node", ex);
            events().reportRuntimeReadinessFailure(
                "InternalError",
                "node runtime failed during restart: " + ex.getMessage()
            );
            return -1;
        }
    }

    void scheduleRestore(String reason) {
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

        final long restoreGeneration = runtimeOperationGate.snapshot();
        promoteServiceForRuntime();
        final AtomicBoolean restoreCompleted = new AtomicBoolean(false);
        mainHandler.postDelayed(() -> {
            if (
                restoreCompleted.get()
                    || !restoreRunning.get()
                    || restoreGeneration != runtimeOperationGate.snapshot()
            ) {
                return;
            }
            events().reportRuntimeReadinessFailure(
                "InternalError",
                "node runtime restore timed out after " + RUNTIME_RESTORE_TIMEOUT_MS + "ms"
            );
        }, RUNTIME_RESTORE_TIMEOUT_MS);
        restoreExecutor.execute(() -> {
            try {
                final boolean restored = runtimeOperationGate.runRestore(restoreGeneration, () -> {
                    if (isNodeRunning()) {
                        resumeRestoredRuntime();
                        return;
                    }

                    final int result = startNodeInternal(persistedConfig);
                    if (result != 0) {
                        events().reportRuntimeReadinessFailure(
                            "InternalError",
                            "node runtime failed to restore after " + reason
                        );
                    }
                });
                if (!restored) {
                    Log.i(TAG, "Skipped stale node runtime restore after " + reason);
                }
            } catch (Exception ex) {
                Logger.error(TAG, "Failed to restore node after " + reason, ex);
                events().reportRuntimeReadinessFailure(
                    "InternalError",
                    "node runtime failed to restore after " + reason + ": " + ex.getMessage()
                );
            } finally {
                restoreCompleted.set(true);
                restoreRunning.set(false);
            }
        });
    }

    boolean shouldBeRunning() {
        if (!preferences.getBoolean(PREF_DESIRED_RUNNING, false)) {
            return false;
        }
        return preferences.getInt(PREF_LAST_BOOT_COUNT, -1) == configResolver.currentBootCount();
    }

    void handleForegroundServiceTimeout(int startId, int foregroundServiceType) {
        runtimeOperationGate.runExplicit(() -> {
            handleForegroundServiceTimeoutInternal(startId, foregroundServiceType);
            return 0;
        });
    }

    private void handleForegroundServiceTimeoutInternal(int startId, int foregroundServiceType) {
        events().reportRuntimeReadinessFailure(
            "InternalError",
            "node runtime foreground service timed out; stopping Reticulum node service. type="
                + foregroundServiceType
        );
        events().stop();
        pluginCoordinator.setNodeRunning(false);
        try {
            ReticulumBridge.stop();
        } catch (Exception ex) {
            Log.w(TAG, "Failed to stop native runtime after foreground service timeout", ex);
        }
        clearDesiredRunning();
        events().refreshLatestRuntimeState();
        events().emitCachedStateToAll();
        stopForegroundAndSelf(startId);
    }

    void updateForegroundNotification() {
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
        NotificationManagerCompat.from(service).notify(
            FOREGROUND_NOTIFICATION_ID,
            buildRuntimeNotification(true, body)
        );
    }

    private ServiceEventCoordinator events() {
        if (eventCoordinator == null) {
            throw new IllegalStateException("Service event coordinator is not attached");
        }
        return eventCoordinator;
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

    private void persistDesiredRunning(RuntimeConfigResolver.ResolvedConfig resolved) {
        preferences.edit()
            .putBoolean(PREF_DESIRED_RUNNING, true)
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
        lastCanonicalConfigJson = "";
    }

    private void resumeUnchangedRuntime() {
        events().start();
        events().refreshLatestRuntimeState();
        service.startForeground(FOREGROUND_NOTIFICATION_ID, buildRuntimeNotification(true));
        events().emitCachedStateToAll();
        events().emitProjectionRefreshSweepToAll();
        pluginCoordinator.setNodeRunning(true);
    }

    private void completeRuntimeStart() {
        events().clearRuntimeReadinessFailure();
        notificationController.primeOperationalState();
        events().refreshLatestRuntimeState();
        events().start();
        service.startForeground(FOREGROUND_NOTIFICATION_ID, buildRuntimeNotification(true));
        events().emitCachedStateToAll();
        events().emitProjectionRefreshSweepToAll();
        pluginCoordinator.setNodeRunning(true);
    }

    private void resumeRestoredRuntime() {
        events().start();
        events().clearRuntimeReadinessFailure();
        events().refreshLatestRuntimeState();
        service.startForeground(FOREGROUND_NOTIFICATION_ID, buildRuntimeNotification(true));
        events().emitCachedStateToAll();
        events().emitProjectionRefreshSweepToAll();
    }

    private void cleanupFailedRuntimeStart() {
        events().stop();
        pluginCoordinator.setNodeRunning(false);
        try {
            ReticulumBridge.stop();
        } catch (Exception ex) {
            Log.w(TAG, "Failed to stop native runtime after start failure", ex);
        }
        clearDesiredRunning();
        events().refreshLatestRuntimeState();
        events().emitCachedStateToAll();
        stopForegroundAndSelf(0);
    }

    private void stopForegroundAndSelf(int startId) {
        try {
            service.stopForeground(ReticulumNodeService.STOP_FOREGROUND_REMOVE);
        } catch (Exception ex) {
            Log.w(TAG, "Failed to remove foreground notification", ex);
        }
        if (startId > 0) {
            service.stopSelf(startId);
        } else {
            service.stopSelf();
        }
    }

    private void promoteServiceForRuntime() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            service.startForeground(FOREGROUND_NOTIFICATION_ID, buildRuntimeNotification(false));
        }
    }

    private Notification buildRuntimeNotification(boolean running) {
        return buildRuntimeNotification(running, buildRuntimeNotificationBody(running));
    }

    private Notification buildRuntimeNotification(boolean running, String body) {
        final Intent launchIntent = new Intent(service, MainActivity.class);
        launchIntent.addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP | Intent.FLAG_ACTIVITY_NEW_TASK);
        final PendingIntent contentIntent = PendingIntent.getActivity(
            service,
            0,
            launchIntent,
            PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE
        );

        final Intent stopIntent = new Intent(service, ReticulumNodeService.class);
        stopIntent.setAction(ReticulumNodeService.ACTION_STOP_SERVICE);
        final PendingIntent stopPendingIntent = PendingIntent.getService(
            service,
            1,
            stopIntent,
            PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE
        );

        final String title = running ? "Mesh node running" : "Starting mesh node";
        final String safeBody = body == null || body.trim().isEmpty()
            ? service.getString(R.string.app_name)
            : body;

        return new NotificationCompat.Builder(service, ServiceNotificationController.RUNTIME_CHANNEL_ID)
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
                JsonPayloads.orFallback(events().latestStatusJson(), "{}")
            );
            final JSONObject sync = new JSONObject(
                JsonPayloads.orFallback(events().latestSyncStatusJson(), "{}")
            );
            final String name = status.optString("name", service.getString(R.string.app_name));
            final String phase = sync.optString("phase", "Idle");
            return name + " | Sync " + phase;
        } catch (JSONException ex) {
            return service.getString(R.string.app_name);
        }
    }
}
