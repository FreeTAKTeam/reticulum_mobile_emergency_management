package network.reticulum.emergency;

import java.util.Locale;

final class RNodeUsbKissControl {
    static final byte FEND = (byte) 0xC0;
    static final byte FESC = (byte) 0xDB;
    static final byte TFEND = (byte) 0xDC;
    static final byte TFESC = (byte) 0xDD;
    static final byte CMD_BT_CTRL = 0x46;
    static final byte CMD_RESET = 0x55;
    static final byte CMD_BT_PIN = 0x62;

    private static final byte BT_CTRL_DISABLE = 0x00;
    private static final byte BT_CTRL_ENABLE = 0x01;
    private static final byte BT_CTRL_PAIRING_MODE = 0x02;
    private static final byte RESET_ESP32 = (byte) 0xF8;

    private RNodeUsbKissControl() {}

    static byte[] bluetoothPairingModeFrame() {
        return commandFrame(CMD_BT_CTRL, BT_CTRL_PAIRING_MODE);
    }

    static byte[] bluetoothDisableFrame() {
        return commandFrame(CMD_BT_CTRL, BT_CTRL_DISABLE);
    }

    static byte[] bluetoothEnableFrame() {
        return commandFrame(CMD_BT_CTRL, BT_CTRL_ENABLE);
    }

    static byte[] hardResetFrame() {
        return commandFrame(CMD_RESET, RESET_ESP32);
    }

    static String decodeBluetoothPin(byte[] payload) {
        if (payload == null || payload.length < 4) {
            return null;
        }
        final int value = ((payload[0] & 0xFF) << 24)
            | ((payload[1] & 0xFF) << 16)
            | ((payload[2] & 0xFF) << 8)
            | (payload[3] & 0xFF);
        return String.format(Locale.US, "%06d", value);
    }

    private static byte[] commandFrame(byte command, byte payload) {
        return new byte[] { FEND, command, payload, FEND };
    }
}
