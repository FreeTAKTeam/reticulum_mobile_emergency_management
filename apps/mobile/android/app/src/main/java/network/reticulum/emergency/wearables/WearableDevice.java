package network.reticulum.emergency.wearables;

import com.getcapacitor.JSArray;
import com.getcapacitor.JSObject;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.UUID;

public final class WearableDevice {
    private final String deviceId;
    private final String address;
    private final String name;
    private final int rssi;
    private final List<UUID> advertisedServices;
    private final boolean bonded;
    private final boolean heartRateSupported;
    private WearableConnectionState connectionState;

    public WearableDevice(
        String deviceId,
        String address,
        String name,
        int rssi,
        List<UUID> advertisedServices,
        boolean bonded,
        boolean heartRateSupported,
        WearableConnectionState connectionState
    ) {
        this.deviceId = deviceId == null ? "" : deviceId;
        this.address = address == null ? "" : address;
        this.name = name == null || name.trim().isEmpty() ? "Generic BLE Heart Rate Device" : name;
        this.rssi = rssi;
        this.advertisedServices = advertisedServices == null
            ? Collections.emptyList()
            : Collections.unmodifiableList(new ArrayList<>(advertisedServices));
        this.bonded = bonded;
        this.heartRateSupported = heartRateSupported;
        this.connectionState = connectionState == null ? WearableConnectionState.DISCOVERED : connectionState;
    }

    public String getDeviceId() {
        return deviceId;
    }

    public String getAddress() {
        return address;
    }

    public String getName() {
        return name;
    }

    public int getRssi() {
        return rssi;
    }

    public List<UUID> getAdvertisedServices() {
        return advertisedServices;
    }

    public boolean isBonded() {
        return bonded;
    }

    public boolean isHeartRateSupported() {
        return heartRateSupported;
    }

    public WearableConnectionState getConnectionState() {
        return connectionState;
    }

    public void setConnectionState(WearableConnectionState connectionState) {
        this.connectionState = connectionState == null ? WearableConnectionState.ERROR : connectionState;
    }

    public JSObject toJson() {
        final JSArray services = new JSArray();
        for (UUID service : advertisedServices) {
            services.put(service.toString());
        }
        final JSObject object = new JSObject();
        object.put("deviceId", deviceId);
        object.put("deviceName", name);
        object.put("rssi", rssi);
        object.put("advertisedServices", services);
        object.put("bonded", bonded);
        object.put("heartRateSupported", heartRateSupported);
        object.put("connectionState", connectionState.name());
        return object;
    }
}
