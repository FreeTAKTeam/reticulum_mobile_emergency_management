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
    static final String ACTION_START_RUNTIME = "network.reticulum.emergency.action.START_NODE";
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
    private PluginCoordinator pluginCoordinator;
    private ServiceNotificationController notificationController;
    private ServiceEventCoordinator eventCoordinator;
    private NodeRuntimeLifecycleController runtimeController;
    private RNodeAndroidTransportManager rnodeTransportManager;

    @Override
    public void onCreate() {
        super.onCreate();
        rnodeTransportManager = new RNodeAndroidTransportManager(this);
        RNodeAndroidTransportManager.install(rnodeTransportManager);
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
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        final String action = intent == null ? null : intent.getAction();
        final RuntimeServiceStartDecision.Command command = RuntimeServiceStartDecision.decide(
            action,
            runtimeController.shouldBeRunning()
        );
        switch (command) {
            case STOP:
                stopNode();
                return START_NOT_STICKY;
            case RESTORE_AFTER_BOOT:
                runtimeController.scheduleRestore("boot");
                break;
            case RESTORE_AFTER_PROCESS_RECREATION:
                runtimeController.scheduleRestore("process recreation");
                break;
            case KEEP_RUNNING:
                break;
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
        if (rnodeTransportManager != null) {
            RNodeAndroidTransportManager.uninstall(rnodeTransportManager);
            rnodeTransportManager = null;
        }
        if (eventCoordinator != null) {
            eventCoordinator.close();
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

    private String latestStatusJson() {
        return eventCoordinator == null ? "{}" : eventCoordinator.latestStatusJson();
    }
}
