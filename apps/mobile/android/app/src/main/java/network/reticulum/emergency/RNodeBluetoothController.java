package network.reticulum.emergency;

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
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.pm.PackageManager;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import android.os.ParcelUuid;
import android.util.Log;

import androidx.core.content.ContextCompat;

import com.getcapacitor.JSArray;
import com.getcapacitor.JSObject;
import com.getcapacitor.PermissionState;
import com.getcapacitor.PluginCall;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.atomic.AtomicBoolean;

// Every public Bluetooth operation is gated by hasPermission/requirePermission
// and also maps SecurityException to a rejected Capacitor call.
@SuppressLint("MissingPermission")
final class RNodeBluetoothController {
    interface EventSink {
        void publish(String eventName, JSObject payload);
    }

    private static final String TAG = "ReticulumNode";
    private static final String RNODE_UART_SERVICE_UUID = "6e400001-b5a3-f393-e0a9-e50e24dcca9e";
    private static final long DEFAULT_SCAN_TIMEOUT_MS = 8_000L;
    private static final long PAIRING_TIMEOUT_MS = 45_000L;

    private final Context context;
    private final EventSink eventSink;

    RNodeBluetoothController(Context context, EventSink eventSink) {
        this.context = context;
        this.eventSink = eventSink;
    }

    boolean hasPermission() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) {
            return true;
        }
        return ContextCompat.checkSelfPermission(context, Manifest.permission.BLUETOOTH_SCAN)
                == PackageManager.PERMISSION_GRANTED
            && ContextCompat.checkSelfPermission(context, Manifest.permission.BLUETOOTH_CONNECT)
                == PackageManager.PERMISSION_GRANTED;
    }

    void resolvePermission(PluginCall call, PermissionState permissionState) {
        final JSObject payload = new JSObject();
        payload.put("bluetooth", permissionState.toString().toLowerCase(Locale.ROOT));
        call.resolve(payload);
    }

    void listPairedDevices(PluginCall call) {
        if (!requirePermission(call)) {
            return;
        }
        final BluetoothAdapter adapter = bluetoothAdapter();
        if (adapter == null) {
            call.reject("Bluetooth is unavailable.");
            return;
        }
        try {
            final Set<BluetoothDevice> bondedDevices = adapter.getBondedDevices();
            final JSArray items = new JSArray();
            for (BluetoothDevice device : bondedDevices) {
                items.put(devicePayload(device, null, null));
            }
            final JSObject payload = new JSObject();
            payload.put("items", items);
            call.resolve(payload);
        } catch (SecurityException ex) {
            call.reject("Bluetooth permission denied.", ex);
        }
    }

    void scanDevices(PluginCall call, String mode) {
        if ("bluetooth_classic".equals(mode)) {
            scanClassicDevices(call);
            return;
        }
        if (!"ble".equals(mode)) {
            call.reject("Unsupported RNode Bluetooth mode: " + mode);
            return;
        }
        scanBleDevices(call);
    }

    private void scanBleDevices(PluginCall call) {
        if (!requirePermission(call)) {
            return;
        }
        final BluetoothAdapter adapter = bluetoothAdapter();
        if (adapter == null || !adapter.isEnabled()) {
            call.reject("Bluetooth is not enabled.");
            return;
        }
        final BluetoothLeScanner scanner = adapter.getBluetoothLeScanner();
        if (scanner == null) {
            call.reject("Bluetooth LE scanning is unavailable.");
            return;
        }

        final long timeoutMs = Math.max(1_000L, call.getLong("timeoutMs", DEFAULT_SCAN_TIMEOUT_MS));
        final Map<String, JSObject> discovered = new LinkedHashMap<>();
        final Handler handler = new Handler(Looper.getMainLooper());
        final AtomicBoolean finished = new AtomicBoolean(false);
        final ScanCallback callback = scanCallback(call, discovered, finished);
        final List<ScanFilter> filters = new ArrayList<>();
        filters.add(
            new ScanFilter.Builder()
                .setServiceUuid(ParcelUuid.fromString(RNODE_UART_SERVICE_UUID))
                .build()
        );
        final ScanSettings settings = new ScanSettings.Builder()
            .setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY)
            .build();
        try {
            scanner.startScan(filters, settings, callback);
        } catch (SecurityException ex) {
            call.reject("Bluetooth permission denied.", ex);
            return;
        }
        handler.postDelayed(() -> finishScan(call, scanner, callback, discovered, finished), timeoutMs);
    }

    void pairDevice(PluginCall call, String mode) {
        if (!requirePermission(call)) {
            return;
        }
        final String id = call.getString("id", call.getString("address", ""));
        if (id == null || id.trim().isEmpty()) {
            call.reject("id is required.");
            return;
        }
        final BluetoothAdapter adapter = bluetoothAdapter();
        if (adapter == null) {
            call.reject("Bluetooth is unavailable.");
            return;
        }
        try {
            final BluetoothDevice device = adapter.getRemoteDevice(id.trim());
            final JSObject payload = new JSObject();
            payload.put("id", device.getAddress());
            payload.put("address", device.getAddress());
            payload.put("paired", device.getBondState() == BluetoothDevice.BOND_BONDED);
            if (device.getBondState() == BluetoothDevice.BOND_BONDED) {
                payload.put("bondState", "bonded");
                call.resolve(payload);
                return;
            }
            if (!createBond(device, mode)) {
                call.reject("Android did not start Bluetooth pairing for this RNode.");
                return;
            }
            payload.put("bondingStarted", true);
            payload.put("bondState", bondStateLabel(device.getBondState()));
            call.resolve(payload);
        } catch (IllegalArgumentException ex) {
            call.reject("Invalid Bluetooth device id.", ex);
        } catch (SecurityException ex) {
            call.reject("Bluetooth permission denied.", ex);
        }
    }

    private void scanClassicDevices(PluginCall call) {
        if (!requirePermission(call)) {
            return;
        }
        final BluetoothAdapter adapter = bluetoothAdapter();
        if (adapter == null || !adapter.isEnabled()) {
            call.reject("Bluetooth is not enabled.");
            return;
        }
        final long timeoutMs = Math.max(1_000L, call.getLong("timeoutMs", DEFAULT_SCAN_TIMEOUT_MS));
        final Map<String, JSObject> discovered = new LinkedHashMap<>();
        final AtomicBoolean finished = new AtomicBoolean(false);
        final Handler handler = new Handler(Looper.getMainLooper());
        try {
            for (BluetoothDevice device : adapter.getBondedDevices()) {
                discovered.put(device.getAddress(), devicePayload(device, null, null));
            }
        } catch (SecurityException error) {
            call.reject("Bluetooth permission denied.", error);
            return;
        }
        final BroadcastReceiver receiver = new BroadcastReceiver() {
            @Override
            public void onReceive(Context receiverContext, Intent intent) {
                if (BluetoothDevice.ACTION_FOUND.equals(intent.getAction())) {
                    final BluetoothDevice device = intentDevice(intent);
                    if (device != null && device.getAddress() != null) {
                        final int rssi = intent.getShortExtra(BluetoothDevice.EXTRA_RSSI, Short.MIN_VALUE);
                        discovered.put(
                            device.getAddress(),
                            devicePayload(device, rssi, null, "bluetooth_classic")
                        );
                    }
                } else if (BluetoothAdapter.ACTION_DISCOVERY_FINISHED.equals(intent.getAction())) {
                    finishClassicScan(call, adapter, this, discovered, finished);
                }
            }
        };
        final IntentFilter filter = new IntentFilter();
        filter.addAction(BluetoothDevice.ACTION_FOUND);
        filter.addAction(BluetoothAdapter.ACTION_DISCOVERY_FINISHED);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            context.registerReceiver(receiver, filter, Context.RECEIVER_NOT_EXPORTED);
        } else {
            context.registerReceiver(receiver, filter);
        }
        try {
            adapter.cancelDiscovery();
            if (!adapter.startDiscovery()) {
                unregisterReceiverQuietly(receiver);
                call.reject("Android did not start Bluetooth Classic discovery.");
                return;
            }
        } catch (SecurityException error) {
            unregisterReceiverQuietly(receiver);
            call.reject("Bluetooth permission denied.", error);
            return;
        }
        handler.postDelayed(
            () -> finishClassicScan(call, adapter, receiver, discovered, finished),
            timeoutMs
        );
    }

    private void finishClassicScan(
        PluginCall call,
        BluetoothAdapter adapter,
        BroadcastReceiver receiver,
        Map<String, JSObject> discovered,
        AtomicBoolean finished
    ) {
        if (!finished.compareAndSet(false, true)) {
            return;
        }
        unregisterReceiverQuietly(receiver);
        try {
            adapter.cancelDiscovery();
        } catch (SecurityException error) {
            call.reject("Bluetooth permission denied.", error);
            return;
        }
        final JSArray items = new JSArray();
        for (JSObject item : discovered.values()) {
            items.put(item);
        }
        final JSObject payload = new JSObject();
        payload.put("items", items);
        call.resolve(payload);
    }

    void pairSelectedDeviceWithPin(PluginCall call, String bluetoothDeviceId, String pin) {
        final BluetoothAdapter adapter = bluetoothAdapter();
        if (adapter == null || !adapter.isEnabled()) {
            resolveManualPairing(
                call,
                pin,
                "Bluetooth is unavailable; enter the PIN in Android Bluetooth settings.",
                false
            );
            return;
        }
        try {
            pairDeviceWithPin(call, adapter.getRemoteDevice(bluetoothDeviceId.trim()), null, pin);
        } catch (IllegalArgumentException ex) {
            resolveManualPairing(
                call,
                pin,
                "Selected Bluetooth RNode address is invalid; enter the PIN in Android Bluetooth settings.",
                false
            );
        } catch (SecurityException ex) {
            call.reject("Bluetooth permission denied.", ex);
        }
    }

    private ScanCallback scanCallback(
        PluginCall call,
        Map<String, JSObject> discovered,
        AtomicBoolean finished
    ) {
        return new ScanCallback() {
            @Override
            public void onScanResult(int callbackType, ScanResult result) {
                addScanResult(discovered, result);
            }

            @Override
            public void onBatchScanResults(List<ScanResult> results) {
                for (ScanResult result : results) {
                    addScanResult(discovered, result);
                }
            }

            @Override
            public void onScanFailed(int errorCode) {
                if (finished.compareAndSet(false, true)) {
                    call.reject("RNode Bluetooth scan failed: " + errorCode);
                }
            }
        };
    }

    private void finishScan(
        PluginCall call,
        BluetoothLeScanner scanner,
        ScanCallback callback,
        Map<String, JSObject> discovered,
        AtomicBoolean finished
    ) {
        if (!finished.compareAndSet(false, true)) {
            return;
        }
        try {
            scanner.stopScan(callback);
        } catch (SecurityException ex) {
            call.reject("Bluetooth permission denied.", ex);
            return;
        }
        final JSArray items = new JSArray();
        for (JSObject item : discovered.values()) {
            items.put(item);
        }
        final JSObject payload = new JSObject();
        payload.put("items", items);
        call.resolve(payload);
    }

    private void pairDeviceWithPin(PluginCall call, BluetoothDevice device, Integer rssi, String pin) {
        if (device == null) {
            call.reject("RNode Bluetooth scan returned no device.");
            return;
        }
        final String address = device.getAddress();
        final AtomicBoolean finished = new AtomicBoolean(false);
        final Handler handler = new Handler(Looper.getMainLooper());
        publishStatus("Pairing with discovered RNode");
        if (device.getBondState() == BluetoothDevice.BOND_BONDED) {
            final JSObject payload = devicePayload(device, rssi, null);
            payload.put("pairingModeStarted", true);
            payload.put("pin", pin);
            payload.put("paired", true);
            payload.put("bondState", "bonded");
            call.resolve(payload);
            return;
        }

        final BroadcastReceiver receiver = pairingReceiver(call, address, rssi, pin, finished);
        final IntentFilter filter = new IntentFilter();
        filter.addAction(BluetoothDevice.ACTION_PAIRING_REQUEST);
        filter.addAction(BluetoothDevice.ACTION_BOND_STATE_CHANGED);
        filter.setPriority(IntentFilter.SYSTEM_HIGH_PRIORITY - 1);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            context.registerReceiver(receiver, filter, Context.RECEIVER_EXPORTED);
        } else {
            context.registerReceiver(receiver, filter);
        }

        try {
            if (!createBondWithPreferredTransport(device) && finished.compareAndSet(false, true)) {
                unregisterReceiverQuietly(receiver);
                resolveManualPairing(call, pin, "Android did not start Bluetooth pairing for this RNode.", false);
                return;
            }
        } catch (SecurityException ex) {
            unregisterReceiverQuietly(receiver);
            call.reject("Bluetooth permission denied.", ex);
            return;
        }
        handler.postDelayed(() -> finishPairingTimeout(call, device, receiver, pin, finished), PAIRING_TIMEOUT_MS);
    }

    private BroadcastReceiver pairingReceiver(
        PluginCall call,
        String address,
        Integer rssi,
        String pin,
        AtomicBoolean finished
    ) {
        return new BroadcastReceiver() {
            @Override
            public void onReceive(Context receiverContext, Intent intent) {
                final BluetoothDevice eventDevice = intentDevice(intent);
                if (eventDevice == null || !address.equalsIgnoreCase(eventDevice.getAddress())) {
                    return;
                }
                if (BluetoothDevice.ACTION_PAIRING_REQUEST.equals(intent.getAction())) {
                    handlePairingRequest(this, eventDevice, pin);
                    return;
                }
                if (!BluetoothDevice.ACTION_BOND_STATE_CHANGED.equals(intent.getAction())) {
                    return;
                }
                final int state = intent.getIntExtra(BluetoothDevice.EXTRA_BOND_STATE, BluetoothDevice.BOND_NONE);
                if (state == BluetoothDevice.BOND_BONDED && finished.compareAndSet(false, true)) {
                    unregisterReceiverQuietly(this);
                    final JSObject payload = devicePayload(eventDevice, rssi, null);
                    payload.put("pairingModeStarted", true);
                    payload.put("pin", pin);
                    payload.put("paired", true);
                    payload.put("bondState", "bonded");
                    call.resolve(payload);
                } else if (state == BluetoothDevice.BOND_NONE && finished.compareAndSet(false, true)) {
                    unregisterReceiverQuietly(this);
                    resolveManualPairing(
                        call,
                        pin,
                        "Android rejected the RNode bond; enter the PIN manually if prompted.",
                        true
                    );
                }
            }
        };
    }

    private void finishPairingTimeout(
        PluginCall call,
        BluetoothDevice device,
        BroadcastReceiver receiver,
        String pin,
        AtomicBoolean finished
    ) {
        if (!finished.compareAndSet(false, true)) {
            return;
        }
        unregisterReceiverQuietly(receiver);
        final JSObject payload = new JSObject();
        payload.put("pairingModeStarted", true);
        payload.put("pin", pin);
        payload.put("paired", device.getBondState() == BluetoothDevice.BOND_BONDED);
        payload.put("manualPinRequired", device.getBondState() != BluetoothDevice.BOND_BONDED);
        payload.put("bondState", bondStateLabel(device.getBondState()));
        payload.put("message", "Timed out waiting for Android to complete RNode Bluetooth pairing.");
        call.resolve(payload);
    }

    private void resolveManualPairing(PluginCall call, String pin, String message, boolean includeBondState) {
        final JSObject payload = new JSObject();
        payload.put("pairingModeStarted", true);
        payload.put("pin", pin);
        payload.put("paired", false);
        payload.put("manualPinRequired", true);
        if (includeBondState) {
            payload.put("bondState", "none");
        }
        payload.put("message", message);
        call.resolve(payload);
    }

    private BluetoothDevice intentDevice(Intent intent) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            return intent.getParcelableExtra(BluetoothDevice.EXTRA_DEVICE, BluetoothDevice.class);
        }
        return intent.getParcelableExtra(BluetoothDevice.EXTRA_DEVICE);
    }

    private void handlePairingRequest(BroadcastReceiver receiver, BluetoothDevice device, String pin) {
        try {
            if (pin != null && !pin.isEmpty()) {
                device.setPin(pin.getBytes(StandardCharsets.UTF_8));
                receiver.abortBroadcast();
                publishStatus("Submitted RNode Bluetooth PIN to Android");
            }
        } catch (SecurityException ex) {
            Log.w(TAG, "Bluetooth permission denied while setting RNode PIN", ex);
        } catch (Exception ex) {
            Log.w(TAG, "Failed to set RNode Bluetooth PIN", ex);
        }
    }

    private boolean createBondWithPreferredTransport(BluetoothDevice device) {
        try {
            final java.lang.reflect.Method createBond = BluetoothDevice.class.getMethod("createBond", int.class);
            final int transport = device.getType() == BluetoothDevice.DEVICE_TYPE_CLASSIC ? 1 : 2;
            return Boolean.TRUE.equals(createBond.invoke(device, transport));
        } catch (Exception ex) {
            Log.w(TAG, "createBond(transport) failed, falling back to createBond()", ex);
            return device.createBond();
        }
    }

    private boolean createBond(BluetoothDevice device, String mode) {
        if (!"ble".equals(mode) && !"bluetooth_classic".equals(mode)) {
            throw new IllegalArgumentException("Unsupported RNode Bluetooth mode: " + mode);
        }
        try {
            final java.lang.reflect.Method createBond = BluetoothDevice.class.getMethod("createBond", int.class);
            final int transport = "bluetooth_classic".equals(mode) ? 1 : 2;
            return Boolean.TRUE.equals(createBond.invoke(device, transport));
        } catch (Exception error) {
            Log.w(TAG, "createBond(transport) failed, falling back to createBond()", error);
            return device.createBond();
        }
    }

    private void unregisterReceiverQuietly(BroadcastReceiver receiver) {
        try {
            context.unregisterReceiver(receiver);
        } catch (IllegalArgumentException cleanupError) {
            // Cleanup is idempotent; Android throws when the receiver was already unregistered.
            Log.d(TAG, "Bluetooth receiver was already unregistered", cleanupError);
        }
    }

    private boolean requirePermission(PluginCall call) {
        if (hasPermission()) {
            return true;
        }
        call.reject("Bluetooth permission denied.");
        return false;
    }

    private BluetoothAdapter bluetoothAdapter() {
        final BluetoothManager manager = (BluetoothManager) context.getSystemService(Context.BLUETOOTH_SERVICE);
        return manager == null ? null : manager.getAdapter();
    }

    private void addScanResult(Map<String, JSObject> discovered, ScanResult result) {
        if (result == null || result.getDevice() == null) {
            return;
        }
        final BluetoothDevice device = result.getDevice();
        final String address = device.getAddress();
        if (address == null || address.isEmpty()) {
            return;
        }
        final String name = result.getScanRecord() == null ? null : result.getScanRecord().getDeviceName();
        discovered.put(address, devicePayload(device, result.getRssi(), name, "ble"));
    }

    private JSObject devicePayload(BluetoothDevice device, Integer rssi, String scannedName) {
        return devicePayload(device, rssi, scannedName, null);
    }

    private JSObject devicePayload(
        BluetoothDevice device,
        Integer rssi,
        String scannedName,
        String discoveredMode
    ) {
        final JSObject item = new JSObject();
        final String address = device.getAddress();
        item.put("id", address);
        item.put("address", address);
        if (rssi != null) {
            item.put("rssi", rssi);
        }
        item.put("paired", device.getBondState() == BluetoothDevice.BOND_BONDED);
        item.put("bondState", bondStateLabel(device.getBondState()));
        String name = scannedName;
        if (name == null || name.trim().isEmpty()) {
            name = device.getName();
        }
        item.put("name", name == null ? "" : name);
        final JSArray supportedModes = new JSArray();
        if ("ble".equals(discoveredMode)
            || device.getType() == BluetoothDevice.DEVICE_TYPE_LE
            || device.getType() == BluetoothDevice.DEVICE_TYPE_DUAL
            || device.getType() == BluetoothDevice.DEVICE_TYPE_UNKNOWN) {
            supportedModes.put("ble");
        }
        if ("bluetooth_classic".equals(discoveredMode)
            || device.getType() == BluetoothDevice.DEVICE_TYPE_CLASSIC
            || device.getType() == BluetoothDevice.DEVICE_TYPE_DUAL) {
            supportedModes.put("bluetooth_classic");
        }
        item.put("supportedModes", supportedModes);
        return item;
    }

    private String bondStateLabel(int state) {
        switch (state) {
            case BluetoothDevice.BOND_BONDED:
                return "bonded";
            case BluetoothDevice.BOND_BONDING:
                return "bonding";
            case BluetoothDevice.BOND_NONE:
            default:
                return "none";
        }
    }

    private void publishStatus(String status) {
        final JSObject payload = new JSObject();
        payload.put("status", status);
        eventSink.publish("rnodeUsbPairingStatus", payload);
    }
}
