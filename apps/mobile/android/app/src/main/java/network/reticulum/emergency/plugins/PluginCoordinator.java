package network.reticulum.emergency.plugins;

import android.app.Notification;
import android.app.NotificationManager;
import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.ServiceConnection;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.os.RemoteException;
import android.util.Log;
import androidx.core.app.NotificationCompat;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Iterator;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import network.reticulum.emergency.R;
import network.reticulum.emergency.ReticulumBridge;
import network.reticulum.emergency.plugin.api.IRemPluginHost;
import network.reticulum.emergency.plugin.api.IRemPluginService;
import network.reticulum.emergency.plugin.api.PluginProtocol;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

public final class PluginCoordinator implements AutoCloseable {
    private static final String TAG = "REM.PluginCoordinator";
    private static final int MAX_RETRIES = 5;
    private final Context context;
    private final Handler mainHandler = new Handler(Looper.getMainLooper());
    private final ExecutorService executor = Executors.newSingleThreadExecutor();
    private final Map<String, PluginConnection> connections = new HashMap<>();
    private volatile boolean nodeRunning;

    public PluginCoordinator(Context context) {
        this.context = context.getApplicationContext();
    }

    public String refresh() {
        try {
            final JSONArray discovered = PluginDiscovery.discover(context);
            final String catalog = ReticulumBridge.syncDiscoveredPluginsJson(
                new JSONObject().put("items", discovered).toString()
            );
            if (catalog != null) {
                reconcile(new JSONObject(catalog).optJSONArray("items"));
            }
            return catalog;
        } catch (Exception error) {
            Log.e(TAG, "Plugin discovery failed", error);
            return null;
        }
    }

    public String listPlugins() {
        return ReticulumBridge.listPluginsJson();
    }

    public String listSensors() {
        return ReticulumBridge.listPluginSensorsJson();
    }

    public void setNodeRunning(boolean running) {
        nodeRunning = running;
        if (running) {
            refresh();
        } else {
            stopAll("node-stopped");
        }
    }

    public void reconcileNow() {
        final String raw = ReticulumBridge.listPluginsJson();
        if (raw == null) {
            return;
        }
        try {
            reconcile(new JSONObject(raw).optJSONArray("items"));
        } catch (JSONException error) {
            Log.e(TAG, "Invalid plugin catalog", error);
        }
    }

    public void dispatchPluginLxmf(JSONObject envelope) {
        final String pluginId = envelope.optString("pluginId", "");
        final PluginConnection connection;
        synchronized (this) {
            connection = connections.get(pluginId);
        }
        if (connection == null || connection.service == null) {
            return;
        }
        try {
            connection.service.onHostEvent(
                new JSONObject()
                    .put("protocolVersion", 1)
                    .put("event", "lxmf.received")
                    .put("payload", envelope)
                    .toString()
            );
        } catch (Exception error) {
            connection.fail("Inbound LXMF dispatch failed: " + error.getMessage());
        }
    }

    public Intent configurationIntent(String pluginId) {
        final String raw = ReticulumBridge.listPluginsJson();
        if (raw == null) {
            return null;
        }
        try {
            final JSONArray items = new JSONObject(raw).optJSONArray("items");
            if (items == null) {
                return null;
            }
            for (int index = 0; index < items.length(); index++) {
                final JSONObject plugin = items.getJSONObject(index);
                if (!pluginId.equals(plugin.optString("pluginId"))
                    || !plugin.optBoolean("trusted")
                    || plugin.isNull("configurationEntrypoint")) {
                    continue;
                }
                return PluginConfigurationActivity.intentFor(context, plugin);
            }
        } catch (JSONException error) {
            Log.e(TAG, "Invalid plugin catalog", error);
        }
        return null;
    }

    private synchronized void reconcile(JSONArray items) throws JSONException {
        final Set<String> desired = new HashSet<>();
        if (items != null && nodeRunning) {
            for (int index = 0; index < items.length(); index++) {
                final JSONObject plugin = items.getJSONObject(index);
                if (plugin.optBoolean("trusted")
                    && plugin.optBoolean("enabled")
                    && !"Incompatible".equals(plugin.optString("state"))
                    && !"Missing".equals(plugin.optString("state"))) {
                    final String pluginId = plugin.getString("pluginId");
                    desired.add(pluginId);
                    bind(plugin);
                }
            }
        }
        final Iterator<Map.Entry<String, PluginConnection>> iterator = connections.entrySet().iterator();
        while (iterator.hasNext()) {
            final Map.Entry<String, PluginConnection> entry = iterator.next();
            if (!desired.contains(entry.getKey())) {
                entry.getValue().stop("disabled-or-unavailable");
                iterator.remove();
            }
        }
    }

    private synchronized void bind(JSONObject plugin) throws JSONException {
        final String pluginId = plugin.getString("pluginId");
        if (connections.containsKey(pluginId)) {
            return;
        }
        final PluginConnection connection = new PluginConnection(plugin);
        connections.put(pluginId, connection);
        connection.bind();
    }

    private synchronized void stopAll(String reason) {
        for (PluginConnection connection : connections.values()) {
            connection.stop(reason);
        }
        connections.clear();
    }

    @Override
    public void close() {
        nodeRunning = false;
        stopAll("host-destroyed");
        executor.shutdownNow();
    }

    private final class PluginConnection implements ServiceConnection, IBinder.DeathRecipient {
        private final JSONObject plugin;
        private final String pluginId;
        private final ComponentName component;
        private IRemPluginService service;
        private int retryCount;
        private boolean bound;
        private boolean active = true;

        PluginConnection(JSONObject plugin) throws JSONException {
            this.plugin = plugin;
            pluginId = plugin.getString("pluginId");
            component = new ComponentName(
                plugin.getString("packageName"),
                plugin.getString("serviceClassName")
            );
        }

        void bind() {
            if (!active || !nodeRunning) {
                return;
            }
            setState("Binding", null);
            final Intent intent = new Intent(PluginProtocol.SERVICE_ACTION).setComponent(component);
            bound = context.bindService(intent, this, Context.BIND_AUTO_CREATE);
            if (!bound) {
                fail("Android refused the plugin service binding");
            }
        }

        @Override
        public void onServiceConnected(ComponentName name, IBinder binder) {
            if (!active || !nodeRunning) {
                cleanup("inactive");
                return;
            }
            service = IRemPluginService.Stub.asInterface(binder);
            try {
                binder.linkToDeath(this, 0);
                final String descriptorJson = service.getDescriptorJson();
                PluginProtocol.requireJsonSize(descriptorJson, "Plugin descriptor");
                final JSONObject descriptor = new JSONObject(descriptorJson);
                if (!pluginId.equals(descriptor.optString("pluginId"))
                    || descriptor.optInt("apiMajor", -1) != plugin.optInt("apiMajor", -1)
                    || descriptor.optInt("apiMinor", -1) != plugin.optInt("apiMinor", -1)) {
                    throw new SecurityException("Binder descriptor does not match manifest identity");
                }
                final JSONObject session = new JSONObject()
                    .put("protocolVersion", PluginProtocol.API_MAJOR)
                    .put("apiMajor", PluginProtocol.API_MAJOR)
                    .put(
                        "apiMinor",
                        Math.min(PluginProtocol.API_MINOR, descriptor.optInt("apiMinor", 0))
                    )
                    .put("sessionId", UUID.randomUUID().toString())
                    .put("hostPackage", context.getPackageName());
                service.start(new HostCallback(this), session.toString());
                retryCount = 0;
                setState("Running", null);
            } catch (Exception error) {
                fail("Plugin start failed: " + error.getMessage());
            }
        }

        @Override
        public void onServiceDisconnected(ComponentName name) {
            fail("Plugin service disconnected");
        }

        @Override
        public void binderDied() {
            mainHandler.post(() -> fail("Plugin process died"));
        }

        void stop(String reason) {
            active = false;
            cleanup(reason);
        }

        private void cleanup(String reason) {
            if (service != null) {
                try {
                    service.stop(reason);
                } catch (RemoteException cleanupError) {
                    Log.d(TAG, "Plugin stop callback failed during cleanup: " + pluginId, cleanupError);
                }
            }
            if (bound) {
                try {
                    context.unbindService(this);
                } catch (IllegalArgumentException cleanupError) {
                    Log.d(TAG, "Plugin service was already unbound: " + pluginId, cleanupError);
                }
            }
            bound = false;
            service = null;
        }

        void fail(String message) {
            cleanup("failed");
            setState("Failed", message);
            if (active && nodeRunning && retryCount < MAX_RETRIES) {
                retryCount += 1;
                final long delayMs = Math.min(60_000L, 1_000L << retryCount);
                mainHandler.postDelayed(this::bind, delayMs);
            }
        }

        void setState(String state, String diagnostic) {
            try {
                ReticulumBridge.setPluginRuntimeStateJson(
                    new JSONObject()
                        .put("pluginId", pluginId)
                        .put("state", state)
                        .put("diagnostic", diagnostic == null ? JSONObject.NULL : diagnostic)
                        .toString()
                );
            } catch (JSONException error) {
                Log.w(TAG, "Unable to publish plugin state for " + pluginId, error);
            }
        }
    }

    private final class HostCallback extends IRemPluginHost.Stub {
        private final PluginConnection connection;

        HostCallback(PluginConnection connection) {
            this.connection = connection;
        }

        @Override
        public void submitRequest(String requestJson) {
            executor.execute(() -> handleHostRequest(connection, requestJson));
        }
    }

    private void handleHostRequest(PluginConnection connection, String requestJson) {
        String response;
        JSONObject request = null;
        try {
            request = PluginProtocol.requireEnvelope(requestJson);
            request.put("pluginId", connection.pluginId);
            response = ReticulumBridge.handlePluginHostRequestJson(request.toString());
            if (response == null) {
                response = PluginProtocol.errorResponse(
                    request.optString("requestId"),
                    "HostError",
                    "REM rejected the plugin request"
                );
            }
            PluginProtocol.requireJsonSize(response, "Plugin host response");
        } catch (Exception error) {
            response = PluginProtocol.errorResponse("", "InvalidRequest", error.getMessage());
        }
        if (request != null
            && "notifications.raise".equals(request.optString("operation"))
            && response != null) {
            try {
                if (new JSONObject(response).optBoolean("ok")) {
                    raiseNotification(connection.pluginId, request.optJSONObject("payload"));
                }
            } catch (JSONException error) {
                Log.w(TAG, "Unable to inspect plugin host response for " + connection.pluginId, error);
            }
        }
        final IRemPluginService service = connection.service;
        if (service != null) {
            try {
                service.onHostResponse(response);
            } catch (RemoteException error) {
                connection.fail("Host response failed: " + error.getMessage());
            }
        }
    }

    private void raiseNotification(String pluginId, JSONObject payload) {
        if (payload == null) {
            return;
        }
        final Notification notification = new NotificationCompat.Builder(
            context,
            "operational-updates"
        )
            .setSmallIcon(R.mipmap.ic_launcher)
            .setContentTitle(payload.optString("title", "Plugin update"))
            .setContentText(payload.optString("body", pluginId))
            .setAutoCancel(true)
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .build();
        final NotificationManager manager = context.getSystemService(NotificationManager.class);
        if (manager != null) {
            try {
                manager.notify(49_000 + Math.floorMod(pluginId.hashCode(), 900), notification);
            } catch (SecurityException error) {
                Log.w(TAG, "Notification permission denied for plugin " + pluginId, error);
            }
        }
    }
}
