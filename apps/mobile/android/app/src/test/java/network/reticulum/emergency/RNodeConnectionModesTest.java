package network.reticulum.emergency;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public class RNodeConnectionModesTest {
    @Test
    public void bluetoothRepairIncludesLegacyBleAndClassicModes() {
        assertTrue(RNodeConnectionModes.usesBluetoothRepair(""));
        assertTrue(RNodeConnectionModes.usesBluetoothRepair("ble"));
        assertTrue(RNodeConnectionModes.usesBluetoothRepair("BluetoothClassic"));
        assertTrue(RNodeConnectionModes.usesBluetoothRepair("spp"));
    }

    @Test
    public void bluetoothRepairExcludesUsbAndTcpModes() {
        assertFalse(RNodeConnectionModes.usesBluetoothRepair("usb"));
        assertFalse(RNodeConnectionModes.usesBluetoothRepair("tcp"));
        assertFalse(RNodeConnectionModes.usesBluetoothRepair("wi-fi"));
    }
}
