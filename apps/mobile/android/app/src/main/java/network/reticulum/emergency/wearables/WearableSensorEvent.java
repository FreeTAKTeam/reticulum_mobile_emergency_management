package network.reticulum.emergency.wearables;

import com.getcapacitor.JSObject;

public final class WearableSensorEvent {
    public static final String TYPE_HEART_RATE = "wearable.heart_rate";
    public static final String SOURCE_BLE_GATT_STANDARD_HR = "ble_gatt_standard_hr";
    public static final String SENSOR_HEART_RATE_BPM = "heart_rate_bpm";

    private final String type;
    private final String source;
    private final String deviceId;
    private final String deviceName;
    private final String deviceModel;
    private final long timestampMs;
    private final String sensorType;
    private final int value;
    private final String unit;
    private final float confidence;
    private final String connectionState;

    public WearableSensorEvent(
        String type,
        String source,
        String deviceId,
        String deviceName,
        String deviceModel,
        long timestampMs,
        String sensorType,
        int value,
        String unit,
        float confidence,
        String connectionState
    ) {
        this.type = type;
        this.source = source;
        this.deviceId = deviceId;
        this.deviceName = deviceName;
        this.deviceModel = deviceModel;
        this.timestampMs = timestampMs;
        this.sensorType = sensorType;
        this.value = value;
        this.unit = unit;
        this.confidence = confidence;
        this.connectionState = connectionState;
    }

    public static WearableSensorEvent heartRate(WearableDevice device, int bpm, String connectionState) {
        return new WearableSensorEvent(
            TYPE_HEART_RATE,
            SOURCE_BLE_GATT_STANDARD_HR,
            device.getDeviceId(),
            device.getName(),
            "generic_ble_heart_rate_device",
            System.currentTimeMillis(),
            SENSOR_HEART_RATE_BPM,
            bpm,
            "bpm",
            0.95f,
            connectionState
        );
    }

    public JSObject toJson() {
        final JSObject object = new JSObject();
        object.put("type", type);
        object.put("source", source);
        object.put("device_id", deviceId);
        object.put("device_name", deviceName);
        object.put("device_model", deviceModel);
        object.put("timestamp_ms", timestampMs);
        object.put("sensor_type", sensorType);
        object.put("value", value);
        object.put("unit", unit);
        object.put("confidence", confidence);
        object.put("connection_state", connectionState);
        return object;
    }
}
