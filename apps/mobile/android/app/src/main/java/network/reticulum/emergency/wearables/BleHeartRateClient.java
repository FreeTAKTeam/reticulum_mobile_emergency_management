package network.reticulum.emergency.wearables;

import android.Manifest;
import android.annotation.SuppressLint;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothGatt;
import android.bluetooth.BluetoothGattCallback;
import android.bluetooth.BluetoothGattCharacteristic;
import android.bluetooth.BluetoothGattDescriptor;
import android.bluetooth.BluetoothGattService;
import android.bluetooth.BluetoothProfile;
import android.content.Context;
import android.content.pm.PackageManager;
import android.os.Build;
import android.util.Log;

import androidx.core.content.ContextCompat;

import java.util.List;
import java.util.UUID;

public final class BleHeartRateClient implements WearableAdapter {
    public static final UUID HEART_RATE_SERVICE_UUID = UUID.fromString("0000180d-0000-1000-8000-00805f9b34fb");
    public static final UUID HEART_RATE_MEASUREMENT_UUID = UUID.fromString("00002a37-0000-1000-8000-00805f9b34fb");
    public static final UUID CLIENT_CONFIGURATION_UUID = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb");
    private static final String TAG = "BleHeartRateClient";
    private static final int MAX_VALID_BPM = 240;

    private final Context context;
    private WearableSensorListener listener;
    private BluetoothGatt gatt;
    private WearableDevice wearableDevice;

    public BleHeartRateClient(Context context, WearableSensorListener listener) {
        this.context = context.getApplicationContext();
        this.listener = listener;
    }

    public static int parseHeartRate(byte[] data) {
        if (data == null || data.length < 2) {
            return -1;
        }

        final int flags = data[0] & 0xFF;
        final boolean is16Bit = (flags & 0x01) != 0;
        final int bpm;
        if (is16Bit) {
            if (data.length < 3) {
                return -1;
            }
            bpm = ((data[2] & 0xFF) << 8) | (data[1] & 0xFF);
        } else {
            bpm = data[1] & 0xFF;
        }
        return isValidBpm(bpm) ? bpm : -1;
    }

    public static boolean isValidBpm(int bpm) {
        return bpm > 0 && bpm <= MAX_VALID_BPM;
    }

    @Override
    public boolean supports(BluetoothDevice device, List<UUID> advertisedServices) {
        return advertisedServices != null && advertisedServices.contains(HEART_RATE_SERVICE_UUID);
    }

    @Override
    @SuppressLint("MissingPermission")
    public void connect(BluetoothDevice device, WearableDevice wearableDevice) {
        if (!hasConnectPermission()) {
            notifyError(BlePermissionHelper.ERROR_MISSING_PERMISSIONS, "Bluetooth connect permission is missing.");
            return;
        }
        disconnect();
        this.wearableDevice = wearableDevice;
        updateState(WearableConnectionState.CONNECTING, "Connecting to BLE GATT Heart Rate device.");
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            gatt = device.connectGatt(context, false, callback, BluetoothDevice.TRANSPORT_LE);
        } else {
            gatt = device.connectGatt(context, false, callback);
        }
    }

    @Override
    @SuppressLint("MissingPermission")
    public void disconnect() {
        if (gatt != null) {
            try {
                gatt.disconnect();
                gatt.close();
            } catch (RuntimeException ex) {
                Log.w(TAG, "BLE GATT disconnect failed", ex);
            }
            gatt = null;
        }
        updateState(WearableConnectionState.DISCONNECTED, "Disconnected from BLE GATT Heart Rate device.");
    }

    @Override
    public void setListener(WearableSensorListener listener) {
        this.listener = listener;
    }

    private final BluetoothGattCallback callback = new BluetoothGattCallback() {
        @Override
        @SuppressLint("MissingPermission")
        public void onConnectionStateChange(BluetoothGatt bluetoothGatt, int status, int newState) {
            if (status != BluetoothGatt.GATT_SUCCESS) {
                updateState(WearableConnectionState.ERROR, "GATT connection failed with status " + status + ".");
                safeClose(bluetoothGatt);
                return;
            }
            if (newState == BluetoothProfile.STATE_CONNECTED) {
                updateState(WearableConnectionState.CONNECTED, "Connected to BLE GATT device.");
                bluetoothGatt.discoverServices();
                return;
            }
            if (newState == BluetoothProfile.STATE_DISCONNECTED) {
                updateState(WearableConnectionState.DISCONNECTED, "BLE GATT device disconnected.");
                safeClose(bluetoothGatt);
            }
        }

        @Override
        @SuppressLint("MissingPermission")
        public void onServicesDiscovered(BluetoothGatt bluetoothGatt, int status) {
            if (status != BluetoothGatt.GATT_SUCCESS) {
                updateState(WearableConnectionState.ERROR, "GATT service discovery failed with status " + status + ".");
                return;
            }
            updateState(WearableConnectionState.SERVICE_DISCOVERED, "BLE GATT services discovered.");

            final BluetoothGattService service = bluetoothGatt.getService(HEART_RATE_SERVICE_UUID);
            if (service == null) {
                updateState(WearableConnectionState.UNSUPPORTED, "Standard BLE Heart Rate Service 0x180D not found.");
                return;
            }
            final BluetoothGattCharacteristic characteristic = service.getCharacteristic(HEART_RATE_MEASUREMENT_UUID);
            if (characteristic == null) {
                updateState(WearableConnectionState.UNSUPPORTED, "Heart Rate Measurement characteristic 0x2A37 not found.");
                return;
            }
            updateState(WearableConnectionState.HEART_RATE_AVAILABLE, "BLE Heart Rate Measurement is available.");
            subscribe(bluetoothGatt, characteristic);
        }

        @Override
        public void onCharacteristicChanged(BluetoothGatt bluetoothGatt, BluetoothGattCharacteristic characteristic) {
            handleMeasurement(characteristic.getValue());
        }

        @Override
        public void onDescriptorWrite(BluetoothGatt bluetoothGatt, BluetoothGattDescriptor descriptor, int status) {
            if (CLIENT_CONFIGURATION_UUID.equals(descriptor.getUuid()) && status == BluetoothGatt.GATT_SUCCESS) {
                updateState(WearableConnectionState.SUBSCRIBED, "Subscribed to BLE Heart Rate Measurement notifications.");
            } else if (CLIENT_CONFIGURATION_UUID.equals(descriptor.getUuid())) {
                updateState(WearableConnectionState.ERROR, "BLE notification subscription failed with status " + status + ".");
            }
        }
    };

    @SuppressLint("MissingPermission")
    private void subscribe(BluetoothGatt bluetoothGatt, BluetoothGattCharacteristic characteristic) {
        if (!bluetoothGatt.setCharacteristicNotification(characteristic, true)) {
            updateState(WearableConnectionState.ERROR, "Unable to enable local BLE notification routing.");
            return;
        }
        final BluetoothGattDescriptor descriptor = characteristic.getDescriptor(CLIENT_CONFIGURATION_UUID);
        if (descriptor == null) {
            updateState(WearableConnectionState.ERROR, "Client Characteristic Configuration descriptor 0x2902 not found.");
            return;
        }
        descriptor.setValue(BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE);
        if (!bluetoothGatt.writeDescriptor(descriptor)) {
            updateState(WearableConnectionState.ERROR, "Unable to write BLE Heart Rate notification descriptor.");
        }
    }

    private void handleMeasurement(byte[] value) {
        final int bpm = parseHeartRate(value);
        if (bpm < 0) {
            notifyError("InvalidHeartRateMeasurement", "Invalid BLE heart-rate payload discarded.");
            return;
        }
        final WearableDevice device = wearableDevice;
        final WearableSensorListener currentListener = listener;
        if (device != null && currentListener != null) {
            currentListener.onSensorEvent(WearableSensorEvent.heartRate(device, bpm, WearableConnectionState.SUBSCRIBED.name()));
        }
    }

    private void updateState(WearableConnectionState state, String detail) {
        final WearableDevice device = wearableDevice;
        if (device != null) {
            device.setConnectionState(state);
        }
        final WearableSensorListener currentListener = listener;
        if (currentListener != null) {
            currentListener.onConnectionStateChanged(device, state, detail);
        }
    }

    @SuppressLint("MissingPermission")
    private void safeClose(BluetoothGatt bluetoothGatt) {
        if (bluetoothGatt == null) {
            return;
        }
        try {
            bluetoothGatt.close();
        } catch (RuntimeException ex) {
            Log.w(TAG, "BLE GATT close failed", ex);
        }
        if (gatt == bluetoothGatt) {
            gatt = null;
        }
    }

    private boolean hasConnectPermission() {
        return Build.VERSION.SDK_INT < Build.VERSION_CODES.S
            || ContextCompat.checkSelfPermission(context, Manifest.permission.BLUETOOTH_CONNECT)
                == PackageManager.PERMISSION_GRANTED;
    }

    private void notifyError(String code, String message) {
        final WearableSensorListener currentListener = listener;
        if (currentListener != null) {
            currentListener.onWearableError(code, message);
        }
    }
}
