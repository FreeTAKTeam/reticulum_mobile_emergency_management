package org.freetakteam.rem.plugin.bleheartrate;

import android.Manifest;
import android.annotation.SuppressLint;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothGatt;
import android.bluetooth.BluetoothGattCallback;
import android.bluetooth.BluetoothGattCharacteristic;
import android.bluetooth.BluetoothGattDescriptor;
import android.bluetooth.BluetoothGattService;
import android.bluetooth.BluetoothManager;
import android.bluetooth.BluetoothProfile;
import android.content.pm.PackageManager;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;
import androidx.core.app.NotificationCompat;
import androidx.core.content.ContextCompat;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.Locale;
import java.util.Set;
import java.util.UUID;
import network.reticulum.emergency.plugin.api.IRemPluginConfigurationCallback;
import network.reticulum.emergency.plugin.api.IRemPluginHost;
import network.reticulum.emergency.plugin.api.RemPluginService;
import org.json.JSONObject;

public final class HeartRatePluginService extends RemPluginService {
    private static final String TAG = "REM.HeartRatePlugin";
    static final UUID HEART_RATE_SERVICE = UUID.fromString("0000180d-0000-1000-8000-00805f9b34fb");
    static final UUID HEART_RATE_MEASUREMENT = UUID.fromString("00002a37-0000-1000-8000-00805f9b34fb");
    static final UUID CCCD = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb");
    private static final String CHANNEL = "rem-ble-heart-rate";
    private static final int NOTIFICATION_ID = 18013;
    private final Handler handler = new Handler(Looper.getMainLooper());
    private final SampleRateLimiter limiter = new SampleRateLimiter();
    private final ReconnectPolicy reconnectPolicy = new ReconnectPolicy();
    private PluginPreferences preferences;
    private volatile IRemPluginHost host;
    private volatile BluetoothGatt gatt;
    private volatile Runnable pendingReconnect;
    private String connectionState = "Disconnected";
    private long lastSharedAtMs;
    private boolean foreground;
    private static volatile HeartRatePluginService instance;

    @Override
    public void onCreate() {
        super.onCreate();
        instance = this;
        preferences = new PluginPreferences(this);
        final NotificationManager manager = getSystemService(NotificationManager.class);
        if (manager != null && Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            manager.createNotificationChannel(
                new NotificationChannel(CHANNEL, "REM heart rate monitoring", NotificationManager.IMPORTANCE_LOW)
            );
        }
    }

    static void requestReconnect() {
        final HeartRatePluginService current = instance;
        if (current == null || current.host == null) return;
        current.reconnectPolicy.reset();
        current.connectSelectedDevice();
    }

    @Override
    protected Set<String> allowedHostCertificateFingerprints() {
        if (BuildConfig.REM_HOST_FINGERPRINTS.trim().isEmpty()) return Collections.emptySet();
        return new HashSet<>(Arrays.asList(BuildConfig.REM_HOST_FINGERPRINTS.toLowerCase(Locale.ROOT).split(",")));
    }

    @Override
    protected Set<String> allowedHostPackageNames() {
        return Collections.singleton(BuildConfig.REM_HOST_PACKAGE);
    }

    @Override
    protected String getDescriptorJson() {
        return "{\"pluginId\":\"org.freetakteam.rem.plugin.ble_heart_rate\",\"apiMajor\":1,\"apiMinor\":0}";
    }

    @Override
    protected void onPluginStart(IRemPluginHost host, String sessionJson) {
        this.host = host;
        reconnectPolicy.reset();
        connectSelectedDevice();
    }

    @Override
    protected void onPluginStop(String reason) {
        host = null;
        cancelPendingReconnect();
        handler.removeCallbacksAndMessages(null);
        disconnectGatt();
        stopMonitoringForeground();
        stopSelf();
    }

    @Override
    protected void onHostEvent(String eventJson) {
        try {
            final JSONObject event = new JSONObject(eventJson);
            if (!"lxmf.received".equals(event.optString("event"))) return;
            final JSONObject envelope = event.getJSONObject("payload");
            if (!"heart_rate_sample".equals(envelope.optString("messageName"))) return;
            final JSONObject sample = envelope.getJSONObject("payload");
            final int bpm = sample.optInt("bpm", -1);
            if (bpm < 1 || bpm > 240) return;
            publishSensor(
                "remote:" + sample.optString("deviceId", "unknown"),
                sample.optString("alias", "Remote heart rate"),
                sample.optString("operatorRnsIdentity", ""),
                bpm,
                sample.optLong("measuredAtMs", System.currentTimeMillis()),
                "REMOTE",
                "remote"
            );
        } catch (Exception ignored) {
        }
    }

    @Override
    protected void onHostResponse(String responseJson) {}

    @Override
    protected void onConfigurationRequest(
        String requestJson,
        IRemPluginConfigurationCallback callback
    ) {
        try {
            final JSONObject request = new JSONObject(requestJson);
            final String type = request.optString("type", "");
            if ("update".equals(type)) {
                preferences.update(request);
                reconnectPolicy.reset();
                connectSelectedDevice();
            }
            if ("action".equals(type) && "permissions.pair".equals(request.optString("action"))) {
                callback.onResponse(
                    new JSONObject()
                        .put("type", "actionResult")
                        .put("activity", new JSONObject().put("className", PairingActivity.class.getName()))
                        .toString()
                );
                return;
            }
            callback.onResponse(preferences.json(connectionState).toString());
        } catch (Exception error) {
            try {
                callback.onResponse(
                    new JSONObject()
                        .put("type", "validationError")
                        .put("message", error.getMessage())
                        .toString()
                );
            } catch (Exception ignored) {
            }
        }
    }

    @SuppressLint("MissingPermission")
    private void connectSelectedDevice() {
        cancelPendingReconnect();
        if (host == null || preferences.address().isEmpty() || !hasConnectPermission()) {
            connectionState = preferences.address().isEmpty() ? "Not paired" : "Permission required";
            return;
        }
        final BluetoothManager manager = getSystemService(BluetoothManager.class);
        final BluetoothAdapter adapter = manager == null ? null : manager.getAdapter();
        if (adapter == null || !adapter.isEnabled()) {
            connectionState = "Bluetooth unavailable";
            scheduleReconnect();
            return;
        }
        disconnectGatt();
        ensureMonitoringForeground();
        connectionState = "Connecting";
        try {
            gatt = adapter.getRemoteDevice(preferences.address()).connectGatt(
                this,
                false,
                callback,
                BluetoothDeviceCompat.transportLe()
            );
        } catch (IllegalArgumentException error) {
            connectionState = "Invalid device";
            stopMonitoringForeground();
        }
    }

    private final BluetoothGattCallback callback = new BluetoothGattCallback() {
        @Override
        @SuppressLint("MissingPermission")
        public void onConnectionStateChange(BluetoothGatt value, int status, int newState) {
            if (!isCurrentGatt(value)) {
                safeClose(value);
                return;
            }
            if (status == BluetoothGatt.GATT_SUCCESS && newState == BluetoothProfile.STATE_CONNECTED) {
                cancelPendingReconnect();
                reconnectPolicy.reset();
                connectionState = "Connected";
                value.discoverServices();
            } else if (newState == BluetoothProfile.STATE_DISCONNECTED || status != BluetoothGatt.GATT_SUCCESS) {
                connectionState = "Disconnected";
                safeClose(value);
                scheduleReconnect();
            }
        }

        @Override
        @SuppressLint("MissingPermission")
        public void onServicesDiscovered(BluetoothGatt value, int status) {
            if (!isCurrentGatt(value)) {
                safeClose(value);
                return;
            }
            final BluetoothGattService service = value.getService(HEART_RATE_SERVICE);
            final BluetoothGattCharacteristic characteristic = service == null
                ? null
                : service.getCharacteristic(HEART_RATE_MEASUREMENT);
            final BluetoothGattDescriptor descriptor = characteristic == null
                ? null
                : characteristic.getDescriptor(CCCD);
            if (status != BluetoothGatt.GATT_SUCCESS || characteristic == null || descriptor == null) {
                connectionState = "Unsupported";
                safeClose(value);
                stopMonitoringForeground();
                return;
            }
            value.setCharacteristicNotification(characteristic, true);
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                value.writeDescriptor(descriptor, BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE);
            } else {
                descriptor.setValue(BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE);
                value.writeDescriptor(descriptor);
            }
        }

        @Override
        public void onDescriptorWrite(BluetoothGatt value, BluetoothGattDescriptor descriptor, int status) {
            if (!isCurrentGatt(value)) {
                safeClose(value);
                return;
            }
            connectionState = status == BluetoothGatt.GATT_SUCCESS ? "Subscribed" : "Subscription failed";
            if (status != BluetoothGatt.GATT_SUCCESS) {
                safeClose(value);
                scheduleReconnect();
            }
        }

        @Override
        public void onCharacteristicChanged(BluetoothGatt value, BluetoothGattCharacteristic characteristic) {
            if (!isCurrentGatt(value)) return;
            handleMeasurement(characteristic.getValue());
        }

        @Override
        public void onCharacteristicChanged(
            BluetoothGatt value,
            BluetoothGattCharacteristic characteristic,
            byte[] bytes
        ) {
            if (!isCurrentGatt(value)) return;
            handleMeasurement(bytes);
        }
    };

    private void handleMeasurement(byte[] bytes) {
        final HeartRateMeasurement measurement = HeartRateMeasurement.parse(bytes);
        final long now = System.currentTimeMillis();
        if (measurement == null || !limiter.accept(now)) return;
        publishSensor(
            preferences.address(),
            preferences.alias(),
            preferences.operatorIdentity(),
            measurement.bpm(),
            now,
            "SUBSCRIBED",
            "local"
        );
        if (preferences.sharingEnabled()
            && SharingPolicy.shouldSend(
                preferences.destination(),
                lastSharedAtMs,
                now,
                preferences.sendIntervalMs()
            )) {
            lastSharedAtMs = now;
            sendLxmf(measurement.bpm(), now);
        }
    }

    private void publishSensor(
        String deviceId,
        String alias,
        String operator,
        int bpm,
        long atMs,
        String state,
        String origin
    ) {
        try {
            submit("sensor.publish", new JSONObject()
                .put("deviceId", deviceId)
                .put("sensorType", "heart_rate_bpm")
                .put("displayName", alias)
                .put("value", bpm)
                .put("unit", "bpm")
                .put("operatorRnsIdentity", operator)
                .put("connectionState", state)
                .put("timestampMs", atMs)
                .put("staleAfterMs", preferences.staleAfterMs())
                .put("origin", origin));
        } catch (Exception ignored) {
        }
    }

    private void sendLxmf(int bpm, long atMs) {
        try {
            final JSONObject sample = new JSONObject()
                .put("bpm", bpm)
                .put("deviceId", preferences.address())
                .put("alias", preferences.alias())
                .put("operatorRnsIdentity", preferences.operatorIdentity())
                .put("measuredAtMs", atMs);
            submit("lxmf.send", new JSONObject()
                .put("pluginId", "org.freetakteam.rem.plugin.ble_heart_rate")
                .put("destinationHex", preferences.destination())
                .put("messageName", "heart_rate_sample")
                .put("payload", sample)
                .put("bodyUtf8", "Heart rate " + bpm + " bpm")
                .put("title", "Heart rate")
                .put("sendMode", new JSONObject().put("Auto", new JSONObject())));
        } catch (Exception ignored) {
        }
    }

    private void submit(String operation, JSONObject payload) {
        final IRemPluginHost current = host;
        if (current == null) return;
        try {
            current.submitRequest(new JSONObject()
                .put("protocolVersion", 1)
                .put("requestId", UUID.randomUUID().toString())
                .put("operation", operation)
                .put("payload", payload)
                .toString());
        } catch (Exception ignored) {
        }
    }

    private synchronized void scheduleReconnect() {
        if (host == null) return;
        cancelPendingReconnect();
        final long delayMs = reconnectPolicy.nextDelayMs();
        if (delayMs < 0L) {
            stopMonitoringForeground();
            return;
        }
        pendingReconnect = () -> {
            pendingReconnect = null;
            connectSelectedDevice();
        };
        handler.postDelayed(pendingReconnect, delayMs);
    }

    private synchronized void cancelPendingReconnect() {
        final Runnable pending = pendingReconnect;
        pendingReconnect = null;
        if (pending != null) handler.removeCallbacks(pending);
    }

    @SuppressLint("MissingPermission")
    private void disconnectGatt() {
        final BluetoothGatt current = gatt;
        gatt = null;
        if (current != null) {
            try {
                current.disconnect();
            } catch (Exception cleanupError) {
                Log.d(TAG, "Ignoring GATT disconnect failure during cleanup", cleanupError);
            }
            safeClose(current);
        }
        connectionState = "Disconnected";
    }

    @SuppressLint("MissingPermission")
    private void safeClose(BluetoothGatt value) {
        try {
            value.close();
        } catch (Exception cleanupError) {
            Log.d(TAG, "Ignoring GATT close failure during cleanup", cleanupError);
        }
        if (gatt == value) gatt = null;
    }

    private boolean isCurrentGatt(BluetoothGatt value) {
        return value != null && value == gatt;
    }

    private boolean hasConnectPermission() {
        return Build.VERSION.SDK_INT < Build.VERSION_CODES.S
            || ContextCompat.checkSelfPermission(this, Manifest.permission.BLUETOOTH_CONNECT)
                == PackageManager.PERMISSION_GRANTED;
    }

    private void ensureMonitoringForeground() {
        if (foreground) return;
        startForeground(
            NOTIFICATION_ID,
            new NotificationCompat.Builder(this, CHANNEL)
                .setSmallIcon(android.R.drawable.stat_sys_data_bluetooth)
                .setContentTitle("REM heart rate monitoring")
                .setContentText(preferences.alias())
                .setOngoing(true)
                .build()
        );
        foreground = true;
    }

    private void stopMonitoringForeground() {
        if (!foreground) return;
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            stopForeground(STOP_FOREGROUND_REMOVE);
        } else {
            @SuppressWarnings("deprecation")
            final boolean removeNotification = true;
            stopForeground(removeNotification);
        }
        foreground = false;
    }

    @Override
    public void onDestroy() {
        if (instance == this) instance = null;
        super.onDestroy();
    }
}
