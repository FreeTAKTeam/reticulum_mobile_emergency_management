package org.freetakteam.rem.plugin.bleheartrate;

import android.bluetooth.BluetoothDevice;

final class BluetoothDeviceCompat {
    private BluetoothDeviceCompat() {}
    static int transportLe() { return BluetoothDevice.TRANSPORT_LE; }
}
