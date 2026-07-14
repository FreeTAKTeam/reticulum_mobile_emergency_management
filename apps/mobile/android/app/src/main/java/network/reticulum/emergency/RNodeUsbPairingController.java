package network.reticulum.emergency;

import android.content.Context;
import android.util.Log;

import com.getcapacitor.JSArray;
import com.getcapacitor.JSObject;
import com.getcapacitor.PluginCall;

import java.util.concurrent.ExecutorService;

final class RNodeUsbPairingController {
    private static final String TAG = "ReticulumNode";

    private final Context context;
    private final ExecutorService executor;
    private final RNodeBluetoothController bluetoothController;
    private final RNodeBluetoothController.EventSink eventSink;
    private volatile RNodeUsbControlManager controlManager;

    RNodeUsbPairingController(
        Context context,
        ExecutorService executor,
        RNodeBluetoothController bluetoothController,
        RNodeBluetoothController.EventSink eventSink
    ) {
        this.context = context;
        this.executor = executor;
        this.bluetoothController = bluetoothController;
        this.eventSink = eventSink;
    }

    void listDevices(PluginCall call) {
        executor.execute(() -> {
            final JSArray items = new JSArray();
            for (RNodeUsbControlManager.UsbDeviceRecord device : manager().listDevices()) {
                items.put(devicePayload(device));
            }
            final JSObject payload = new JSObject();
            payload.put("items", items);
            call.resolve(payload);
        });
    }

    void requestPermission(PluginCall call) {
        final int deviceId = call.getInt("deviceId", -1);
        if (deviceId < 0) {
            call.reject("deviceId is required.");
            return;
        }
        try {
            manager().requestPermission(deviceId, granted -> {
                final JSObject payload = new JSObject();
                payload.put("deviceId", deviceId);
                payload.put("granted", granted);
                call.resolve(payload);
            });
        } catch (RuntimeException ex) {
            call.reject("USB permission request failed.", ex);
        }
    }

    void startBluetoothPairing(PluginCall call) {
        if (!bluetoothController.hasPermission()) {
            call.reject("Bluetooth permission denied.");
            return;
        }
        final int deviceId = call.getInt("deviceId", -1);
        if (deviceId < 0) {
            call.reject("deviceId is required.");
            return;
        }
        final String bluetoothDeviceId = call.getString("bluetoothDeviceId", "");
        executor.execute(() -> startBluetoothPairing(call, deviceId, bluetoothDeviceId));
    }

    void cancelBluetoothPairing(PluginCall call) {
        manager().cancel();
        final int deviceId = call.getInt("deviceId", -1);
        if (deviceId >= 0) {
            executor.execute(() -> {
                try {
                    manager().exitBluetoothPairingMode(deviceId);
                } catch (Exception ex) {
                    Log.w(TAG, "Failed to exit RNode Bluetooth pairing mode", ex);
                }
            });
        }
        call.resolve();
    }

    void close() {
        final RNodeUsbControlManager current = controlManager;
        if (current != null) {
            current.cancel();
        }
    }

    private void startBluetoothPairing(PluginCall call, int deviceId, String bluetoothDeviceId) {
        try {
            final RNodeUsbControlManager.PairingModeResult pairingMode = manager().enterBluetoothPairingMode(
                deviceId,
                new RNodeUsbControlManager.PairingModeListener() {
                    @Override
                    public void onStatus(String status) {
                        publishValue("rnodeUsbPairingStatus", "status", status);
                    }

                    @Override
                    public void onPin(String pin) {
                        publishValue("rnodeUsbPairingPin", "pin", pin);
                    }
                }
            );
            if (pairingMode.pin == null || pairingMode.pin.trim().isEmpty()) {
                final JSObject payload = pairingPayload(pairingMode.pairingModeStarted, null);
                payload.put("manualPinRequired", true);
                call.resolve(payload);
                return;
            }
            if (bluetoothDeviceId == null || bluetoothDeviceId.trim().isEmpty()) {
                final JSObject payload = pairingPayload(pairingMode.pairingModeStarted, pairingMode.pin);
                payload.put("manualPinRequired", true);
                payload.put(
                    "message",
                    "RNode pairing mode started. Select the matching Bluetooth RNode before using USB-assisted auto-pairing."
                );
                call.resolve(payload);
                return;
            }
            bluetoothController.pairSelectedDeviceWithPin(call, bluetoothDeviceId, pairingMode.pin);
        } catch (Exception ex) {
            call.reject("USB-assisted RNode pairing failed.", ex);
        }
    }

    private RNodeUsbControlManager manager() {
        RNodeUsbControlManager current = controlManager;
        if (current == null) {
            synchronized (this) {
                current = controlManager;
                if (current == null) {
                    current = new RNodeUsbControlManager(context);
                    controlManager = current;
                }
            }
        }
        return current;
    }

    private JSObject devicePayload(RNodeUsbControlManager.UsbDeviceRecord device) {
        final JSObject payload = new JSObject();
        payload.put("deviceId", device.deviceId);
        payload.put("vendorId", device.vendorId);
        payload.put("productId", device.productId);
        payload.put("deviceName", device.deviceName == null ? "" : device.deviceName);
        payload.put("manufacturerName", device.manufacturerName == null ? "" : device.manufacturerName);
        payload.put("productName", device.productName == null ? "" : device.productName);
        payload.put("serialNumber", device.serialNumber == null ? "" : device.serialNumber);
        payload.put("hasPermission", device.hasPermission);
        return payload;
    }

    private JSObject pairingPayload(boolean pairingModeStarted, String pin) {
        final JSObject payload = new JSObject();
        payload.put("pairingModeStarted", pairingModeStarted);
        if (pin != null) {
            payload.put("pin", pin);
        }
        payload.put("paired", false);
        return payload;
    }

    private void publishValue(String eventName, String key, String value) {
        final JSObject payload = new JSObject();
        payload.put(key, value);
        eventSink.publish(eventName, payload);
    }
}
