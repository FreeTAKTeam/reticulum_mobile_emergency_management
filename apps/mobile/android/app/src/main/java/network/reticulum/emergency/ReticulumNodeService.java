package network.reticulum.emergency;

import android.app.Activity;
import android.app.Application;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.os.Binder;
import android.os.Bundle;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.util.Log;

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
    private static final String PREF_WATCH_STATUS_SERVER_ENABLED = "watchStatusServerEnabled";
    private static final String PREF_WATCH_STATUS_SERVER_PORT = "watchStatusServerPort";
    static final String ACTION_RESTORE_AFTER_BOOT = "network.reticulum.emergency.action.RESTORE_AFTER_BOOT";
    static final String ACTION_STOP_SERVICE = "network.reticulum.emergency.action.STOP_NODE";

    private final IBinder binder = new LocalBinder();
    private final AtomicBoolean appUiForeground = new AtomicBoolean(false);
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
    private SosPlatformCoordinator sosPlatformCoordinator;
    private RemWatchStatusServer watchStatusServer;
    private PluginCoordinator pluginCoordinator;
    private ServiceNotificationController notificationController;
    private ServiceEventCoordinator eventCoordinator;
    private NodeRuntimeLifecycleController runtimeController;

    @Override
    public void onCreate() {
        super.onCreate();
        preferences = getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE);
        final RuntimeConfigResolver configResolver = new RuntimeConfigResolver(this);
        NodeRuntimeLifecycleController.initializeStorage(
            configResolver.resolveStorageDir("").getAbsolutePath()
        );
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
        runtimeController = new NodeRuntimeLifecycleController(
            this,
            preferences,
            mainHandler,
            restoreExecutor,
            notificationController,
            pluginCoordinator,
            configResolver
        );
        eventCoordinator = new ServiceEventCoordinator(
            mainHandler,
            notificationController,
            pluginCoordinator,
            sosPlatformCoordinator,
            this::isAppUiForeground,
            runtimeController::updateForegroundNotification
        );
        runtimeController.attachEventCoordinator(eventCoordinator);
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
            runtimeController.scheduleRestore("boot");
            return START_STICKY;
        }

        if (runtimeController.shouldBeRunning()) {
            runtimeController.scheduleRestore("process recreation");
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
        runtimeController.handleForegroundServiceTimeout(startId, 0);
    }

    @Override
    public void onTimeout(int startId, int foregroundServiceType) {
        runtimeController.handleForegroundServiceTimeout(startId, foregroundServiceType);
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
        return runtimeController.startNode(configJson);
    }

    public int stopNode() {
        return runtimeController.stopNode();
    }

    public int restartNode(String configJson) {
        return runtimeController.restartNode(configJson);
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

    private String latestStatusJson() {
        return eventCoordinator == null ? "{}" : eventCoordinator.latestStatusJson();
    }
}
