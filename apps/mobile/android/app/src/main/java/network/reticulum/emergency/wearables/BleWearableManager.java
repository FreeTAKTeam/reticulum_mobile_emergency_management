package network.reticulum.emergency.wearables;

import android.Manifest;
import android.annotation.SuppressLint;
import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothManager;
import android.bluetooth.le.BluetoothLeScanner;
import android.bluetooth.le.ScanCallback;
import android.bluetooth.le.ScanFilter;
import android.bluetooth.le.ScanResult;
import android.bluetooth.le.ScanSettings;
import android.content.Context;
import android.content.pm.PackageManager;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import android.os.ParcelUuid;
import android.util.Log;

import androidx.core.content.ContextCompat;

import com.getcapacitor.JSArray;
import com.getcapacitor.JSObject;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.UUID;

import network.reticulum.emergency.ReticulumBridge;

public final class BleWearableManager implements WearableSensorListener {
    private static final String TAG = "BleWearableManager";
    private static final long DEFAULT_SCAN_TIMEOUT_MS = 15_000L;

    private final Context context;
    private final Handler mainHandler = new Handler(Looper.getMainLooper());
    private final Map<String, WearableDevice> devicesById = new LinkedHashMap<>();
    private final BleHeartRateClient heartRateClient;
    private WearableSensorListener listener;
    private BluetoothLeScanner scanner;
    private boolean scanning;

    public BleWearableManager(Context context, WearableSensorListener listener) {
        this.context = context.getApplicationContext();
        this.listener = listener;
        this.heartRateClient = new BleHeartRateClient(context, this);
    }

    public void setListener(WearableSensorListener listener) {
        this.listener = listener;
    }

    public boolean isScanning() {
        return scanning;
    }

    public JSObject permissionState() {
        final JSArray missing = new JSArray();
        for (String permission : BlePermissionHelper.missingRuntimePermissions(context)) {
            missing.put(permission);
        }
        final JSObject object = new JSObject();
        object.put("granted", missing.length() == 0);
        object.put("missing", missing);
        object.put("androidApi", Build.VERSION.SDK_INT);
        return object;
    }

    @SuppressLint("MissingPermission")
    public JSObject listBondedDevices() {
        if (!BlePermissionHelper.hasConnectPermission(context)) {
            return errorPayload(BlePermissionHelper.ERROR_MISSING_PERMISSIONS, "Bluetooth connect permission is missing.");
        }
        final BluetoothAdapter adapter = adapter();
        if (adapter == null) {
            return errorPayload(BlePermissionHelper.ERROR_BLUETOOTH_UNAVAILABLE, "Bluetooth is not available on this device.");
        }
        final Set<BluetoothDevice> bondedDevices = adapter.getBondedDevices();
        final JSArray items = new JSArray();
        for (BluetoothDevice device : bondedDevices) {
            final WearableDevice wearable = toWearableDevice(device, 0, Collections.emptyList(), true, false);
            devicesById.put(wearable.getDeviceId(), wearable);
            items.put(wearable.toJson());
        }
        final JSObject object = new JSObject();
        object.put("items", items);
        return object;
    }

    @SuppressLint("MissingPermission")
    public JSObject startScan(long timeoutMs) {
        if (!BlePermissionHelper.hasScanPermission(context)) {
            return errorPayload(BlePermissionHelper.ERROR_MISSING_PERMISSIONS, "Bluetooth scan permission is missing.");
        }
        final BluetoothAdapter adapter = adapter();
        if (adapter == null) {
            return errorPayload(BlePermissionHelper.ERROR_BLUETOOTH_UNAVAILABLE, "Bluetooth is not available on this device.");
        }
        if (!adapter.isEnabled()) {
            return errorPayload(BlePermissionHelper.ERROR_BLUETOOTH_DISABLED, "Bluetooth is disabled.");
        }
        final BluetoothLeScanner nextScanner = adapter.getBluetoothLeScanner();
        if (nextScanner == null) {
            return errorPayload(BlePermissionHelper.ERROR_BLUETOOTH_UNAVAILABLE, "Bluetooth LE scanner is unavailable.");
        }

        stopScan("restart");
        scanner = nextScanner;
        final ScanFilter filter = new ScanFilter.Builder()
            .setServiceUuid(new ParcelUuid(BleHeartRateClient.HEART_RATE_SERVICE_UUID))
            .build();
        final ScanSettings settings = new ScanSettings.Builder()
            .setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY)
            .build();
        scanner.startScan(Collections.singletonList(filter), settings, scanCallback);
        scanning = true;
        final long effectiveTimeoutMs = Math.max(1_000L, timeoutMs <= 0 ? DEFAULT_SCAN_TIMEOUT_MS : timeoutMs);
        mainHandler.postDelayed(() -> stopScan("timeout"), effectiveTimeoutMs);

        final JSObject object = new JSObject();
        object.put("scanning", true);
        object.put("timeoutMs", effectiveTimeoutMs);
        return object;
    }

    @SuppressLint("MissingPermission")
    public JSObject stopScan(String reason) {
        if (scanner != null && scanning && BlePermissionHelper.hasScanPermission(context)) {
            try {
                scanner.stopScan(scanCallback);
            } catch (RuntimeException ex) {
                Log.w(TAG, "BLE scan stop failed", ex);
            }
        }
        scanner = null;
        final boolean wasScanning = scanning;
        scanning = false;
        if (wasScanning) {
            onScanStopped(reason == null ? "stopped" : reason);
        }
        final JSObject object = new JSObject();
        object.put("scanning", false);
        object.put("reason", reason == null ? "stopped" : reason);
        return object;
    }

    @SuppressLint("MissingPermission")
    public JSObject connect(String deviceId) {
        if (!BlePermissionHelper.hasConnectPermission(context)) {
            return errorPayload(BlePermissionHelper.ERROR_MISSING_PERMISSIONS, "Bluetooth connect permission is missing.");
        }
        final WearableDevice wearable = devicesById.get(deviceId);
        if (wearable == null) {
            return errorPayload("WearableDeviceNotFound", "Wearable device has not been discovered or listed as bonded.");
        }
        final BluetoothAdapter adapter = adapter();
        if (adapter == null) {
            return errorPayload(BlePermissionHelper.ERROR_BLUETOOTH_UNAVAILABLE, "Bluetooth is not available on this device.");
        }
        try {
            final BluetoothDevice device = adapter.getRemoteDevice(wearable.getAddress());
            heartRateClient.connect(device, wearable);
            final JSObject object = new JSObject();
            object.put("device", wearable.toJson());
            return object;
        } catch (IllegalArgumentException ex) {
            return errorPayload("WearableDeviceNotFound", "Wearable device address is invalid.");
        }
    }

    public JSObject disconnect() {
        heartRateClient.disconnect();
        final JSObject object = new JSObject();
        object.put("disconnected", true);
        return object;
    }

    public JSObject statusJson() {
        final JSArray items = new JSArray();
        for (WearableDevice device : devicesById.values()) {
            items.put(device.toJson());
        }
        final JSObject object = new JSObject();
        object.put("scanning", scanning);
        object.put("items", items);
        return object;
    }

    public void close() {
        stopScan("closed");
        heartRateClient.disconnect();
    }

    @Override
    public void onDeviceDiscovered(WearableDevice device) {
        if (device == null) {
            return;
        }
        devicesById.put(device.getDeviceId(), device);
        final WearableSensorListener currentListener = listener;
        if (currentListener != null) {
            currentListener.onDeviceDiscovered(device);
        }
    }

    @Override
    public void onConnectionStateChanged(WearableDevice device, WearableConnectionState state, String detail) {
        final WearableSensorListener currentListener = listener;
        if (currentListener != null) {
            currentListener.onConnectionStateChanged(device, state, detail);
        }
    }

    @Override
    public void onSensorEvent(WearableSensorEvent event) {
        final String payload = event.toJson().toString();
        final int result = ReticulumBridge.ingestWearableSensorEventJson(payload);
        if (result != 0) {
            onWearableError("RustBridgeUnavailable", "Wearable event could not be forwarded to Rust.");
            return;
        }
        final WearableSensorListener currentListener = listener;
        if (currentListener != null) {
            currentListener.onSensorEvent(event);
        }
    }

    @Override
    public void onScanStopped(String reason) {
        final WearableSensorListener currentListener = listener;
        if (currentListener != null) {
            currentListener.onScanStopped(reason);
        }
    }

    @Override
    public void onWearableError(String code, String message) {
        final WearableSensorListener currentListener = listener;
        if (currentListener != null) {
            currentListener.onWearableError(code, message);
        }
    }

    private final ScanCallback scanCallback = new ScanCallback() {
        @Override
        public void onScanResult(int callbackType, ScanResult result) {
            handleScanResult(result);
        }

        @Override
        public void onBatchScanResults(List<ScanResult> results) {
            for (ScanResult result : results) {
                handleScanResult(result);
            }
        }

        @Override
        public void onScanFailed(int errorCode) {
            scanning = false;
            onWearableError("BleScanFailed", "BLE scan failed with code " + errorCode + ".");
            onScanStopped("error");
        }
    };

    @SuppressLint("MissingPermission")
    private void handleScanResult(ScanResult result) {
        if (result == null || result.getDevice() == null) {
            return;
        }
        final List<UUID> services = advertisedServices(result);
        final boolean supportsHeartRate = services.contains(BleHeartRateClient.HEART_RATE_SERVICE_UUID);
        final WearableDevice device = toWearableDevice(
            result.getDevice(),
            result.getRssi(),
            services,
            result.getDevice().getBondState() == BluetoothDevice.BOND_BONDED,
            supportsHeartRate
        );
        onDeviceDiscovered(device);
    }

    private List<UUID> advertisedServices(ScanResult result) {
        if (result.getScanRecord() == null || result.getScanRecord().getServiceUuids() == null) {
            return Collections.emptyList();
        }
        final List<UUID> services = new ArrayList<>();
        for (ParcelUuid uuid : result.getScanRecord().getServiceUuids()) {
            services.add(uuid.getUuid());
        }
        return services;
    }

    @SuppressLint("MissingPermission")
    private WearableDevice toWearableDevice(
        BluetoothDevice device,
        int rssi,
        List<UUID> advertisedServices,
        boolean bonded,
        boolean heartRateSupported
    ) {
        final String address = device.getAddress();
        final String name = safeDeviceName(device);
        return new WearableDevice(
            stableDeviceId(address),
            address,
            name,
            rssi,
            advertisedServices,
            bonded,
            heartRateSupported,
            WearableConnectionState.DISCOVERED
        );
    }

    @SuppressLint("MissingPermission")
    private String safeDeviceName(BluetoothDevice device) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S
            && ContextCompat.checkSelfPermission(context, Manifest.permission.BLUETOOTH_CONNECT)
                != PackageManager.PERMISSION_GRANTED) {
            return "Generic BLE Heart Rate Device";
        }
        final String name = device.getName();
        return name == null || name.trim().isEmpty() ? "Generic BLE Heart Rate Device" : name;
    }

    private BluetoothAdapter adapter() {
        final BluetoothManager manager = (BluetoothManager) context.getSystemService(Context.BLUETOOTH_SERVICE);
        return manager == null ? null : manager.getAdapter();
    }

    private static String stableDeviceId(String address) {
        final String value = address == null ? "" : address.trim().toUpperCase(Locale.US);
        try {
            final MessageDigest digest = MessageDigest.getInstance("SHA-256");
            final byte[] hash = digest.digest(value.getBytes(StandardCharsets.UTF_8));
            final StringBuilder builder = new StringBuilder("ble-hr-");
            for (int i = 0; i < 12 && i < hash.length; i++) {
                builder.append(String.format(Locale.US, "%02x", hash[i]));
            }
            return builder.toString();
        } catch (NoSuchAlgorithmException ex) {
            return "ble-hr-" + Math.abs(value.hashCode());
        }
    }

    private JSObject errorPayload(String code, String message) {
        final JSObject object = new JSObject();
        object.put("errorCode", code);
        object.put("message", message);
        return object;
    }
}
