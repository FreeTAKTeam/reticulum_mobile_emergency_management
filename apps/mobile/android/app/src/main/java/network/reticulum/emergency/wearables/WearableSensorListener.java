package network.reticulum.emergency.wearables;

public interface WearableSensorListener {
    void onDeviceDiscovered(WearableDevice device);

    void onConnectionStateChanged(WearableDevice device, WearableConnectionState state, String detail);

    void onSensorEvent(WearableSensorEvent event);

    void onScanStopped(String reason);

    void onWearableError(String code, String message);
}
