package network.reticulum.emergency.plugins;

import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;
import android.content.pm.ResolveInfo;
import android.content.pm.ServiceInfo;
import android.os.Bundle;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Set;
import network.reticulum.emergency.plugin.api.CallerCertificateVerifier;
import network.reticulum.emergency.plugin.api.PluginProtocol;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

public final class PluginDiscovery {
    public static final String META_PLUGIN_ID = "rem.plugin.id";
    public static final String META_DISPLAY_NAME = "rem.plugin.displayName";
    public static final String META_VERSION = "rem.plugin.version";
    public static final String META_API_MAJOR = "rem.plugin.apiMajor";
    public static final String META_API_MINOR = "rem.plugin.apiMinor";
    public static final String META_CAPABILITIES = "rem.plugin.capabilities";
    public static final String META_MESSAGES = "rem.plugin.messages";
    public static final String META_CONFIG_ENTRYPOINT = "rem.plugin.configurationEntrypoint";

    private PluginDiscovery() {}

    public static JSONArray discover(Context context) throws JSONException {
        final PackageManager manager = context.getPackageManager();
        final Intent query = new Intent(PluginProtocol.SERVICE_ACTION);
        final List<ResolveInfo> services = manager.queryIntentServices(query, PackageManager.GET_META_DATA);
        final JSONArray result = new JSONArray();
        for (ResolveInfo resolved : services) {
            final ServiceInfo service = resolved.serviceInfo;
            if (service == null || !service.exported || service.metaData == null) {
                continue;
            }
            final JSONObject descriptor = descriptorFromService(context, service);
            if (descriptor != null) {
                result.put(descriptor);
            }
        }
        return result;
    }

    private static JSONObject descriptorFromService(Context context, ServiceInfo service) {
        try {
            final Bundle meta = service.metaData;
            final String pluginId = text(meta, META_PLUGIN_ID);
            final String displayName = text(meta, META_DISPLAY_NAME);
            final String version = text(meta, META_VERSION);
            final int apiMajor = meta.getInt(META_API_MAJOR, -1);
            final int apiMinor = meta.getInt(META_API_MINOR, 0);
            if (!pluginId.matches("[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)+")
                || displayName.isEmpty()
                || !version.matches("[0-9]+\\.[0-9]+\\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?")
                || apiMajor < 0
                || apiMinor < 0) {
                return null;
            }
            final Set<String> currentFingerprintSet = CallerCertificateVerifier.currentPackageFingerprints(
                context,
                service.packageName
            );
            if (currentFingerprintSet.size() != 1) {
                return null;
            }
            final String currentFingerprint = currentFingerprintSet.iterator().next();
            final List<String> fingerprints = new ArrayList<>(
                CallerCertificateVerifier.packageCertificateHistory(context, service.packageName)
            );
            Collections.sort(fingerprints);
            final JSONArray history = new JSONArray();
            for (String fingerprint : fingerprints) {
                history.put(fingerprint);
            }
            final JSONObject capabilities = new JSONObject(
                text(meta, META_CAPABILITIES, "{}")
            );
            final JSONArray messages = new JSONArray(text(meta, META_MESSAGES, "[]"));
            final JSONArray permissions = new JSONArray();
            final PackageInfo packageInfo = context.getPackageManager().getPackageInfo(
                service.packageName,
                PackageManager.GET_PERMISSIONS
            );
            if (packageInfo.requestedPermissions != null) {
                for (String permission : packageInfo.requestedPermissions) {
                    permissions.put(permission);
                }
            }
            final String configEntrypoint = text(meta, META_CONFIG_ENTRYPOINT);
            return new JSONObject()
                .put("pluginId", pluginId)
                .put("displayName", displayName)
                .put("version", version)
                .put("apiMajor", apiMajor)
                .put("apiMinor", apiMinor)
                .put("packageName", service.packageName)
                .put("serviceClassName", service.name)
                .put("publisherFingerprint", currentFingerprint)
                .put("publisherHistory", history)
                .put("androidPermissions", permissions)
                .put("declaredCapabilities", capabilities)
                .put("messages", messages)
                .put("configurationEntrypoint", configEntrypoint.isEmpty() ? JSONObject.NULL : configEntrypoint);
        } catch (Exception error) {
            android.util.Log.w(
                "REM.PluginDiscovery",
                "Ignoring malformed plugin metadata from " + service.packageName,
                error
            );
            return null;
        }
    }

    private static String text(Bundle meta, String key) {
        return text(meta, key, "");
    }

    private static String text(Bundle meta, String key, String fallback) {
        final Object value = meta.get(key);
        return value == null ? fallback : String.valueOf(value).trim();
    }
}
