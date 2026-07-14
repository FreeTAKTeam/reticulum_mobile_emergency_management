package network.reticulum.emergency;

import android.Manifest;
import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothDevice;
import android.content.Context;
import android.content.pm.PackageManager;
import android.os.Build;
import android.provider.Settings;
import android.util.Log;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.io.File;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Iterator;
import java.util.List;
import java.util.Locale;

final class RuntimeConfigResolver {
    private static final String TAG = "ReticulumNodeService";

    private final Context context;

    RuntimeConfigResolver(Context context) {
        this.context = context;
    }

    ResolvedConfig resolve(String rawConfigJson) throws JSONException {
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

    File resolveStorageDir(String rawStorageDir) {
        final String normalized = rawStorageDir == null ? "" : rawStorageDir.trim();
        final File filesDir = context.getFilesDir();
        if (normalized.isEmpty()) {
            return new File(filesDir, "reticulum-mobile");
        }

        final File candidate = new File(normalized);
        return candidate.isAbsolute() ? candidate : new File(filesDir, normalized);
    }

    int currentBootCount() {
        try {
            return Settings.Global.getInt(context.getContentResolver(), Settings.Global.BOOT_COUNT);
        } catch (Settings.SettingNotFoundException ex) {
            return 0;
        }
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
            return context.checkSelfPermission(Manifest.permission.BLUETOOTH_CONNECT)
                == PackageManager.PERMISSION_GRANTED;
        }
        return context.checkSelfPermission(Manifest.permission.BLUETOOTH)
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

    static String normalizeBluetoothId(String value) {
        if (value == null) {
            return "";
        }
        return value.trim().replace(":", "").replace("-", "").toLowerCase(Locale.US);
    }

    static String canonicalize(Object value) throws JSONException {
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

    static final class ResolvedConfig {
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
