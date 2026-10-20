package org.freetakteam.rem.plugin.watchstatus;

import android.content.Intent;
import android.os.Handler;
import android.os.Looper;
import android.os.SystemClock;
import android.util.Log;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.Locale;
import java.util.Set;
import java.util.concurrent.atomic.AtomicBoolean;
import network.reticulum.emergency.plugin.api.IRemPluginConfigurationCallback;
import network.reticulum.emergency.plugin.api.IRemPluginHost;
import network.reticulum.emergency.plugin.api.RemPluginService;
import org.json.JSONObject;

public final class WatchStatusPluginService extends RemPluginService {
    private static final String TAG = "REM.WatchStatusPlugin";
    private static final long REFRESH_MS = 1_000L;
    private static final long STALE_MS = 5_000L;
    private final Handler handler = new Handler(Looper.getMainLooper());
    private final AtomicBoolean requestPending = new AtomicBoolean(false);
    private final WatchStatusServer server = new WatchStatusServer();
    private final HostRequestBroker requests = new HostRequestBroker();
    private final RadarAccess radarAccess = new RadarAccess(SystemClock::elapsedRealtime);
    private final RadarProvider radar = new RadarProvider(requests, radarAccess, System::currentTimeMillis, SystemClock::elapsedRealtime);
    private volatile IRemPluginHost host;
    private volatile long generation;
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
        return "{\"pluginId\":\"org.freetakteam.rem.plugin.watch_status\",\"apiMajor\":1,\"apiMinor\":3}";
    }

    @Override protected synchronized void onPluginStart(IRemPluginHost host, String sessionJson) {
        try {
            if (new JSONObject(sessionJson).optInt("apiMinor", 0) < 3) {
                throw new IllegalStateException("REM plugin API 1.3 is required");
            }
            onPluginStop("host-restarted");
            this.host = host;
            requests.start(host::submitRequest);
            applySettings();
            handler.removeCallbacks(refresh);
            handler.post(refresh);
        } catch (Exception error) {
            onPluginStop("start-failed");
        }
    }

    @Override protected synchronized void onPluginStop(String reason) {
        host = null;
        generation++;
        radarAccess.revoke();
        requests.stop();
        latestSnapshot = null;
        latestSnapshotAtMs = 0;
        requestPending.set(false);
        handler.removeCallbacks(refresh);
        server.stop();
    }

    @Override protected void onHostEvent(String eventJson) {}

    @Override protected void onHostResponse(String responseJson) {
        requests.accept(responseJson);
    }

    @Override public boolean onUnbind(Intent intent) {
        onPluginStop("host-unbound");
        return super.onUnbind(intent);
    }

    @Override public void onDestroy() {
        onPluginStop("service-destroyed");
        requests.close();
        super.onDestroy();
    }

    @Override protected synchronized void onConfigurationRequest(String requestJson, IRemPluginConfigurationCallback callback) {
        try {
            final JSONObject request = new JSONObject(requestJson);
            if ("update".equals(request.optString("type"))) {
                settings.update(request);
                applySettings();
            } else if ("renewRadar".equals(request.optString("type"))) {
                if (host == null || !settings.enabled() || !settings.radarEnabled()) {
                    throw new IllegalStateException("Enable the server and radar access first");
                }
                radarAccess.renew();
            }
            final JSONObject state = settings.json(server.isRunning(), server.bindError(), snapshotAgeMs());
            final JSONObject session = radarAccess.configuration();
            for (String key : new String[] { "radarToken", "radarSessionActive", "radarExpiresInMs" }) {
                state.put(key, session.get(key));
            }
            callback.onResponse(state.toString());
        } catch (Exception error) {
            try {
                callback.onResponse(new JSONObject().put("type", "validationError").put("message", error.getMessage()).toString());
            } catch (Exception callbackError) {
                Log.w(TAG, "Failed to return configuration validation error", callbackError);
            }
        }
    }

    private void applySettings() {
        radarAccess.revoke();
        if (host != null && settings.enabled() && settings.radarEnabled()) radarAccess.renew();
        server.apply(host != null && settings.enabled(), settings.port(), this::currentPayload, radar);
    }

    private synchronized void requestSnapshot() {
        final IRemPluginHost activeHost = host;
        if (activeHost == null || !requestPending.compareAndSet(false, true)) return;
        final long requestGeneration = generation;
        requests.request("operational.snapshot", new JSONObject()).whenComplete((snapshot, error) -> {
            synchronized (WatchStatusPluginService.this) {
                if (generation != requestGeneration || host != activeHost) return;
                if (error == null) {
                    latestSnapshot = snapshot;
                    latestSnapshotAtMs = System.currentTimeMillis();
                } else if (error instanceof RadarFailure && "PermissionDenied".equals(((RadarFailure) error).code)) {
                    latestSnapshot = null;
                    latestSnapshotAtMs = 0;
                }
                requestPending.set(false);
            }
        });
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

}
