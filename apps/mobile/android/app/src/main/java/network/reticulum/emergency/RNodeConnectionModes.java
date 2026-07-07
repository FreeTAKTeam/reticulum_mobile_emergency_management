package network.reticulum.emergency;

import java.util.Locale;

final class RNodeConnectionModes {
    private RNodeConnectionModes() {}

    static boolean usesBluetoothRepair(String rawMode) {
        final String mode = rawMode == null
            ? ""
            : rawMode.trim().toLowerCase(Locale.US).replace('-', '_').replace(' ', '_');
        switch (mode) {
            case "":
            case "ble":
            case "bluetooth_le":
            case "le":
            case "gatt":
            case "bluetooth_classic":
            case "bluetoothclassic":
            case "classic":
            case "spp":
            case "rfcomm":
            case "bluetooth":
                return true;
            default:
                return false;
        }
    }
}
