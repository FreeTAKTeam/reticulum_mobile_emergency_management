package network.reticulum.emergency;

import android.annotation.SuppressLint;
import android.bluetooth.BluetoothDevice;

import com.getcapacitor.JSArray;
import com.getcapacitor.JSObject;

@SuppressLint("MissingPermission")
final class RNodeBluetoothDevicePayload {
    private RNodeBluetoothDevicePayload() {}

    static JSObject from(BluetoothDevice device, Integer rssi, String scannedName) {
        return from(device, rssi, scannedName, null);
    }

    static JSObject from(
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

    static String bondStateLabel(int state) {
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
}
