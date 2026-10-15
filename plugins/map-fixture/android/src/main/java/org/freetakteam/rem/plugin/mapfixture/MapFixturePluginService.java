package org.freetakteam.rem.plugin.mapfixture;

import android.util.Log;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import java.util.UUID;
import network.reticulum.emergency.plugin.api.CallerCertificateVerifier;
import network.reticulum.emergency.plugin.api.IRemPluginConfigurationCallback;
import network.reticulum.emergency.plugin.api.IRemPluginHost;
import network.reticulum.emergency.plugin.api.RemPluginService;

/** Binder wrapper around {@link MapFixtureFlow}; all protocol decisions live in the flow. */
public final class MapFixturePluginService extends RemPluginService {
    private static final String TAG = "REM.MapFixturePlugin";
    private final MapFixtureFlow flow = new MapFixtureFlow(() -> UUID.randomUUID().toString());
    private IRemPluginHost host;

    @Override protected Set<String> allowedHostCertificateFingerprints() {
        if (BuildConfig.REM_HOST_FINGERPRINTS.trim().isEmpty()) {
            // Debug fixture: trust the signer of the installed REM debug build.
            return CallerCertificateVerifier.packageFingerprints(this, BuildConfig.REM_HOST_PACKAGE);
        }
        return new HashSet<>(Arrays.asList(BuildConfig.REM_HOST_FINGERPRINTS.toLowerCase(Locale.ROOT).split(",")));
    }

    @Override protected Set<String> allowedHostPackageNames() {
        return Set.of(BuildConfig.REM_HOST_PACKAGE);
    }

    @Override protected String getDescriptorJson() {
        return "{\"pluginId\":\"org.freetakteam.rem.plugin.map_fixture\",\"apiMajor\":1,\"apiMinor\":3}";
    }

    @Override protected synchronized void onPluginStart(IRemPluginHost host, String sessionJson) {
        this.host = host;
        try {
            submit(flow.start(sessionJson));
        } catch (Exception error) {
            Log.w(TAG, "Map fixture start failed", error);
        }
    }

    @Override protected synchronized void onPluginStop(String reason) {
        host = null;
        flow.stop();
    }

    @Override protected synchronized void onHostEvent(String eventJson) {
        try {
            submit(flow.onHostEvent(eventJson));
        } catch (Exception error) {
            Log.w(TAG, "Map fixture rejected a host event", error);
        }
    }

    @Override protected synchronized void onHostResponse(String responseJson) {
        try {
            submit(flow.onHostResponse(responseJson));
            if (!flow.violations().isEmpty()) {
                Log.e(TAG, "REM delivered unselected map objects: " + flow.violations());
            }
        } catch (Exception error) {
            Log.w(TAG, "Map fixture rejected a host response", error);
        }
    }

    @Override protected void onConfigurationRequest(String requestJson, IRemPluginConfigurationCallback callback) {}

    private void submit(List<String> requests) throws android.os.RemoteException {
        final IRemPluginHost activeHost = host;
        if (activeHost == null) {
            return;
        }
        for (String request : requests) {
            activeHost.submitRequest(request);
        }
    }
}
