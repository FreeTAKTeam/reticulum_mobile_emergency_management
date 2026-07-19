package org.freetakteam.rem.plugin.watchstatus;

import android.content.Intent;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.Locale;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicBoolean;
import network.reticulum.emergency.plugin.api.IRemPluginConfigurationCallback;
import network.reticulum.emergency.plugin.api.IRemPluginHost;
import network.reticulum.emergency.plugin.api.PluginProtocol;
import network.reticulum.emergency.plugin.api.RemPluginService;
import org.json.JSONObject;

public final class WatchStatusPluginService extends RemPluginService {
    private static final String TAG = "REM.WatchStatusPlugin";
    private static final long REFRESH_MS = 1_000L;
    private static final long STALE_MS = 5_000L;
    private final Handler handler = new Handler(Looper.getMainLooper());
    private final AtomicBoolean requestPending = new AtomicBoolean(false);
    private final WatchStatusServer server = new WatchStatusServer();
    private volatile IRemPluginHost host;
    private volatile String pendingRequestId = "";
    private volatile JSONObject latestSnapshot;
    private volatile long latestSnapshotAtMs;
    private WatchStatusSettings settings;

    private final Runnable refresh = new Runnable() {
        @Override public void run() {
            requestSnapshot();
            if (host != null) handler.postDelayed(this, REFRESH_MS);
        }
    };

    @Override public void onCreate() {
        super.onCreate();
        settings = new WatchStatusSettings(this);
    }

    @Override protected Set<String> allowedHostCertificateFingerprints() {
        if (BuildConfig.REM_HOST_FINGERPRINTS.trim().isEmpty()) return Collections.emptySet();
        return new HashSet<>(Arrays.asList(BuildConfig.REM_HOST_FINGERPRINTS.toLowerCase(Locale.ROOT).split(",")));
    }

    @Override protected Set<String> allowedHostPackageNames() {
        return Collections.singleton(BuildConfig.REM_HOST_PACKAGE);
    }

    @Override protected String getDescriptorJson() {
        return "{\"pluginId\":\"org.freetakteam.rem.plugin.watch_status\",\"apiMajor\":1,\"apiMinor\":1}";
    }

    @Override protected void onPluginStart(IRemPluginHost host, String sessionJson) {
        try {
            if (new JSONObject(sessionJson).optInt("apiMinor", 0) < 1) {
                throw new IllegalStateException("REM plugin API 1.1 is required");
            }
            this.host = host;
            applySettings();
            handler.removeCallbacks(refresh);
            handler.post(refresh);
        } catch (Exception error) {
            this.host = null;
            server.stop();
        }
    }

    @Override protected void onPluginStop(String reason) {
        host = null;
        pendingRequestId = "";
        requestPending.set(false);
        handler.removeCallbacks(refresh);
        server.stop();
    }

    @Override protected void onHostEvent(String eventJson) {}

    @Override protected void onHostResponse(String responseJson) {
        boolean matchesPendingRequest = false;
        try {
            PluginProtocol.requireJsonSize(responseJson, "Operational snapshot response");
            final JSONObject response = new JSONObject(responseJson);
            if (!responseMatches(pendingRequestId, response)) return;
            matchesPendingRequest = true;
            if (response.optBoolean("ok")) {
                final JSONObject result = response.optJSONObject("result");
                if (result != null) {
                    latestSnapshot = result;
                    latestSnapshotAtMs = System.currentTimeMillis();
                }
            }
        } catch (Exception ignored) {
        } finally {
            if (matchesPendingRequest) {
                pendingRequestId = "";
                requestPending.set(false);
            }
        }
    }

    @Override public boolean onUnbind(Intent intent) {
        onPluginStop("host-unbound");
        return super.onUnbind(intent);
    }

    @Override public void onDestroy() {
        onPluginStop("service-destroyed");
        super.onDestroy();
    }

    @Override protected void onConfigurationRequest(String requestJson, IRemPluginConfigurationCallback callback) {
        try {
            final JSONObject request = new JSONObject(requestJson);
            if ("update".equals(request.optString("type"))) {
                settings.update(request);
                applySettings();
            }
            callback.onResponse(settings.json(server.isRunning(), server.bindError(), snapshotAgeMs()).toString());
        } catch (Exception error) {
            try {
                callback.onResponse(new JSONObject().put("type", "validationError").put("message", error.getMessage()).toString());
            } catch (Exception callbackError) {
                Log.w(TAG, "Failed to return configuration validation error", callbackError);
            }
        }
    }

    private void applySettings() {
        server.apply(settings.enabled(), settings.port(), this::currentPayload);
    }

    private void requestSnapshot() {
        final IRemPluginHost activeHost = host;
        if (activeHost == null || !requestPending.compareAndSet(false, true)) return;
        try {
            pendingRequestId = UUID.randomUUID().toString();
            activeHost.submitRequest(new JSONObject()
                .put("protocolVersion", 1)
                .put("requestId", pendingRequestId)
                .put("operation", "operational.snapshot")
                .put("payload", new JSONObject())
                .toString());
        } catch (Exception error) {
            pendingRequestId = "";
            requestPending.set(false);
        }
    }

    private String currentPayload() {
        final long now = System.currentTimeMillis();
        final JSONObject snapshot = latestSnapshot;
        if (snapshot == null || now - latestSnapshotAtMs > STALE_MS) {
            return WatchStatusPayload.error("Operational snapshot unavailable or stale", now);
        }
        try {
            return WatchStatusPayload.build(snapshot, now);
        } catch (Exception error) {
            return WatchStatusPayload.error(error.getMessage(), now);
        }
    }

    private long snapshotAgeMs() {
        return latestSnapshotAtMs == 0L ? -1L : Math.max(0L, System.currentTimeMillis() - latestSnapshotAtMs);
    }

    static boolean responseMatches(String pendingId, JSONObject response) {
        return pendingId != null
            && !pendingId.isEmpty()
            && pendingId.equals(response.optString("requestId", ""));
    }
}
