package network.reticulum.emergency.wearables;

import android.bluetooth.BluetoothDevice;

import java.util.List;
import java.util.UUID;

public interface WearableAdapter {
    boolean supports(BluetoothDevice device, List<UUID> advertisedServices);

    void connect(BluetoothDevice device, WearableDevice wearableDevice);

    void disconnect();

    void setListener(WearableSensorListener listener);
}
