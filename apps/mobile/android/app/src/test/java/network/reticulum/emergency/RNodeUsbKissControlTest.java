package network.reticulum.emergency;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNull;

import org.junit.Test;

public class RNodeUsbKissControlTest {
    @Test
    public void encodesBluetoothControlFrames() {
        assertArrayEquals(
            new byte[] { (byte) 0xC0, 0x46, 0x02, (byte) 0xC0 },
            RNodeUsbKissControl.bluetoothPairingModeFrame()
        );
        assertArrayEquals(
            new byte[] { (byte) 0xC0, 0x46, 0x00, (byte) 0xC0 },
            RNodeUsbKissControl.bluetoothDisableFrame()
        );
        assertArrayEquals(
            new byte[] { (byte) 0xC0, 0x46, 0x01, (byte) 0xC0 },
            RNodeUsbKissControl.bluetoothEnableFrame()
        );
    }

    @Test
    public void decodesBluetoothPinFrame() {
        assertEquals("123456", RNodeUsbKissControl.decodeBluetoothPin(new byte[] { 0x00, 0x01, (byte) 0xE2, 0x40 }));
        assertEquals("000042", RNodeUsbKissControl.decodeBluetoothPin(new byte[] { 0x00, 0x00, 0x00, 0x2A }));
        assertNull(RNodeUsbKissControl.decodeBluetoothPin(new byte[] { 0x00, 0x01 }));
    }
}
