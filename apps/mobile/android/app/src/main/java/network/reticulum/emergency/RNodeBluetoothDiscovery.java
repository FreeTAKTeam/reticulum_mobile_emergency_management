package network.reticulum.emergency;

import android.annotation.SuppressLint;
import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.le.BluetoothLeScanner;
import android.bluetooth.le.ScanCallback;
import android.bluetooth.le.ScanFilter;
import android.bluetooth.le.ScanResult;
import android.bluetooth.le.ScanSettings;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import android.os.ParcelUuid;
import android.util.Log;

import com.getcapacitor.JSArray;
import com.getcapacitor.JSObject;
import com.getcapacitor.PluginCall;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.atomic.AtomicBoolean;

/** Mode-aware BLE and Classic discovery, separate from bonding and live sessions. */
@SuppressLint("MissingPermission")
final class RNodeBluetoothDiscovery {
    private static final String TAG = "ReticulumNode";
    private static final String RNODE_UART_SERVICE_UUID =
        "6e400001-b5a3-f393-e0a9-e50e24dcca9e";
    private static final long DEFAULT_SCAN_TIMEOUT_MS = 8_000L;

    private final Context context;

    RNodeBluetoothDiscovery(Context context) {
        this.context = context;
    }

    void scan(PluginCall call, BluetoothAdapter adapter, String mode) {
        if (adapter == null || !adapter.isEnabled()) {
            call.reject("Bluetooth is not enabled.");
            return;
        }
        if ("bluetooth_classic".equals(mode)) {
            scanClassic(call, adapter);
        } else if ("ble".equals(mode)) {
            scanBle(call, adapter);
        } else {
            call.reject("Unsupported RNode Bluetooth mode: " + mode);
        }
    }

    private void scanBle(PluginCall call, BluetoothAdapter adapter) {
        final BluetoothLeScanner scanner = adapter.getBluetoothLeScanner();
        if (scanner == null) {
            call.reject("Bluetooth LE scanning is unavailable.");
            return;
        }
        final long timeoutMs = Math.max(1_000L, call.getLong("timeoutMs", DEFAULT_SCAN_TIMEOUT_MS));
        final Map<String, JSObject> discovered = new LinkedHashMap<>();
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
        } catch (SecurityException error) {
            call.reject("Bluetooth permission denied.", error);
            return;
        }
        new Handler(Looper.getMainLooper()).postDelayed(
            () -> finishBle(call, scanner, callback, discovered, finished),
            timeoutMs
        );
    }

    private void scanClassic(PluginCall call, BluetoothAdapter adapter) {
        final long timeoutMs = Math.max(1_000L, call.getLong("timeoutMs", DEFAULT_SCAN_TIMEOUT_MS));
        final Map<String, JSObject> discovered = new LinkedHashMap<>();
        final AtomicBoolean finished = new AtomicBoolean(false);
        try {
            for (BluetoothDevice device : adapter.getBondedDevices()) {
                discovered.put(
                    device.getAddress(),
                    RNodeBluetoothDevicePayload.from(device, null, null)
                );
            }
        } catch (SecurityException error) {
            call.reject("Bluetooth permission denied.", error);
            return;
        }
        try {
            adapter.cancelDiscovery();
        } catch (SecurityException error) {
            call.reject("Bluetooth permission denied.", error);
            return;
        }
        final BroadcastReceiver receiver = classicReceiver(call, adapter, discovered, finished);
        final IntentFilter filter = new IntentFilter();
        filter.addAction(BluetoothDevice.ACTION_FOUND);
        filter.addAction(BluetoothAdapter.ACTION_DISCOVERY_FINISHED);
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                context.registerReceiver(receiver, filter, Context.RECEIVER_NOT_EXPORTED);
            } else {
                context.registerReceiver(receiver, filter);
            }
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
        new Handler(Looper.getMainLooper()).postDelayed(
            () -> finishClassic(call, adapter, receiver, discovered, finished),
            timeoutMs
        );
    }

    private BroadcastReceiver classicReceiver(
        PluginCall call,
        BluetoothAdapter adapter,
        Map<String, JSObject> discovered,
        AtomicBoolean finished
    ) {
        return new BroadcastReceiver() {
            @Override
            public void onReceive(Context receiverContext, Intent intent) {
                if (BluetoothDevice.ACTION_FOUND.equals(intent.getAction())) {
                    final BluetoothDevice device = intentDevice(intent);
                    if (device != null && device.getAddress() != null) {
                        final int rssi = intent.getShortExtra(
                            BluetoothDevice.EXTRA_RSSI,
                            Short.MIN_VALUE
                        );
                        discovered.put(
                            device.getAddress(),
                            RNodeBluetoothDevicePayload.from(
                                device,
                                rssi,
                                null,
                                "bluetooth_classic"
                            )
                        );
                    }
                } else if (BluetoothAdapter.ACTION_DISCOVERY_FINISHED.equals(intent.getAction())) {
                    finishClassic(call, adapter, this, discovered, finished);
                }
            }
        };
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

    private void addScanResult(Map<String, JSObject> discovered, ScanResult result) {
        if (result == null || result.getDevice() == null) {
            return;
        }
        final BluetoothDevice device = result.getDevice();
        final String address = device.getAddress();
        if (address == null || address.isEmpty()) {
            return;
        }
        final String name = result.getScanRecord() == null
            ? null
            : result.getScanRecord().getDeviceName();
        discovered.put(
            address,
            RNodeBluetoothDevicePayload.from(device, result.getRssi(), name, "ble")
        );
    }

    private void finishBle(
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
        } catch (SecurityException error) {
            call.reject("Bluetooth permission denied.", error);
            return;
        }
        resolveItems(call, discovered);
    }

    private void finishClassic(
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
        resolveItems(call, discovered);
    }

    private void resolveItems(PluginCall call, Map<String, JSObject> discovered) {
        final JSArray items = new JSArray();
        for (JSObject item : discovered.values()) {
            items.put(item);
        }
        final JSObject payload = new JSObject();
        payload.put("items", items);
        call.resolve(payload);
    }

    private BluetoothDevice intentDevice(Intent intent) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            return intent.getParcelableExtra(BluetoothDevice.EXTRA_DEVICE, BluetoothDevice.class);
        }
        return intent.getParcelableExtra(BluetoothDevice.EXTRA_DEVICE);
    }

    private void unregisterReceiverQuietly(BroadcastReceiver receiver) {
        try {
            context.unregisterReceiver(receiver);
        } catch (IllegalArgumentException cleanupError) {
            Log.d(TAG, "Bluetooth receiver was already unregistered", cleanupError);
        }
    }
}
