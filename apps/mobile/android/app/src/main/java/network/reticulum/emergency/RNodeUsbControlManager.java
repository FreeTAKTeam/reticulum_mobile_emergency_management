package network.reticulum.emergency;

import android.app.PendingIntent;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.hardware.usb.UsbDevice;
import android.hardware.usb.UsbManager;
import android.os.Build;
import android.util.Log;

import com.hoho.android.usbserial.driver.CdcAcmSerialDriver;
import com.hoho.android.usbserial.driver.Ch34xSerialDriver;
import com.hoho.android.usbserial.driver.Cp21xxSerialDriver;
import com.hoho.android.usbserial.driver.FtdiSerialDriver;
import com.hoho.android.usbserial.driver.ProbeTable;
import com.hoho.android.usbserial.driver.ProlificSerialDriver;
import com.hoho.android.usbserial.driver.UsbSerialDriver;
import com.hoho.android.usbserial.driver.UsbSerialPort;
import com.hoho.android.usbserial.driver.UsbSerialProber;

import java.io.IOException;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicBoolean;

final class RNodeUsbControlManager {
    interface PermissionCallback {
        void onResult(boolean granted);
    }

    interface PairingModeListener {
        void onStatus(String status);

        void onPin(String pin);
    }

    static final class UsbDeviceRecord {
        final int deviceId;
        final int vendorId;
        final int productId;
        final String deviceName;
        final String manufacturerName;
        final String productName;
        final String serialNumber;
        final boolean hasPermission;

        UsbDeviceRecord(UsbDevice device, boolean hasPermission) {
            this.deviceId = device.getDeviceId();
            this.vendorId = device.getVendorId();
            this.productId = device.getProductId();
            this.deviceName = device.getDeviceName();
            this.manufacturerName = safeManufacturerName(device);
            this.productName = safeProductName(device);
            this.serialNumber = safeSerialNumber(device);
            this.hasPermission = hasPermission;
        }
    }

    static final class PairingModeResult {
        final boolean pairingModeStarted;
        final String pin;

        PairingModeResult(boolean pairingModeStarted, String pin) {
            this.pairingModeStarted = pairingModeStarted;
            this.pin = pin;
        }
    }

    private static final String TAG = "RNodeUsbControl";
    private static final String ACTION_USB_PERMISSION = "network.reticulum.emergency.RNODE_USB_PERMISSION";
    private static final int BAUD_RATE = 115_200;
    private static final int READ_TIMEOUT_MS = 200;
    private static final long PIN_WAIT_TIMEOUT_MS = 3_000L;

    private final Context context;
    private final UsbManager usbManager;
    private final AtomicBoolean cancelled = new AtomicBoolean(false);
    private volatile UsbSerialPort activePort;

    RNodeUsbControlManager(Context context) {
        this.context = context.getApplicationContext();
        this.usbManager = (UsbManager) this.context.getSystemService(Context.USB_SERVICE);
    }

    List<UsbDeviceRecord> listDevices() {
        final List<UsbDeviceRecord> records = new ArrayList<>();
        for (UsbSerialDriver driver : findDrivers()) {
            final UsbDevice device = driver.getDevice();
            records.add(new UsbDeviceRecord(device, usbManager.hasPermission(device)));
        }
        return records;
    }

    boolean hasPermission(int deviceId) {
        final UsbDevice device = findUsbDevice(deviceId);
        return device != null && usbManager.hasPermission(device);
    }

    void requestPermission(int deviceId, PermissionCallback callback) {
        final UsbDevice device = findUsbDevice(deviceId);
        if (device == null) {
            callback.onResult(false);
            return;
        }
        if (usbManager.hasPermission(device)) {
            callback.onResult(true);
            return;
        }

        final BroadcastReceiver receiver =
            new BroadcastReceiver() {
                @Override
                public void onReceive(Context receiverContext, Intent intent) {
                    if (!ACTION_USB_PERMISSION.equals(intent.getAction())) {
                        return;
                    }
                    try {
                        receiverContext.unregisterReceiver(this);
                    } catch (IllegalArgumentException cleanupError) {
                        // Receiver may already be unregistered if Android sends a duplicate result.
                        Log.d("ReticulumNode", "USB permission receiver was already unregistered", cleanupError);
                    }
                    final UsbDevice resultDevice =
                        Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU
                            ? intent.getParcelableExtra(UsbManager.EXTRA_DEVICE, UsbDevice.class)
                            : intent.getParcelableExtra(UsbManager.EXTRA_DEVICE);
                    final boolean granted =
                        resultDevice != null
                            && resultDevice.getDeviceId() == deviceId
                            && intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false);
                    callback.onResult(granted);
                }
            };

        final IntentFilter filter = new IntentFilter(ACTION_USB_PERMISSION);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            context.registerReceiver(receiver, filter, Context.RECEIVER_NOT_EXPORTED);
        } else {
            context.registerReceiver(receiver, filter);
        }

        final int flags =
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.S
                ? PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_MUTABLE
                : PendingIntent.FLAG_UPDATE_CURRENT;
        final Intent permissionIntent = new Intent(ACTION_USB_PERMISSION).setPackage(context.getPackageName());
        final PendingIntent intent = PendingIntent.getBroadcast(context, deviceId, permissionIntent, flags);
        usbManager.requestPermission(device, intent);
    }

    PairingModeResult enterBluetoothPairingMode(int deviceId, PairingModeListener listener) throws IOException {
        cancelled.set(false);
        listener.onStatus("Opening RNode USB serial control channel");
        final UsbSerialDriver driver = findDriver(deviceId);
        if (driver == null) {
            throw new IOException("No compatible RNode USB serial device found.");
        }
        final UsbDevice device = driver.getDevice();
        if (!usbManager.hasPermission(device)) {
            throw new IOException("USB permission denied.");
        }
        if (driver.getPorts().isEmpty()) {
            throw new IOException("USB serial device has no serial ports.");
        }

        UsbSerialPort port = null;
        try {
            port = driver.getPorts().get(0);
            activePort = port;
            final android.hardware.usb.UsbDeviceConnection connection = usbManager.openDevice(device);
            if (connection == null) {
                throw new IOException("Android could not open the USB device.");
            }
            port.open(connection);
            port.setParameters(BAUD_RATE, UsbSerialPort.DATABITS_8, UsbSerialPort.STOPBITS_1, UsbSerialPort.PARITY_NONE);
            listener.onStatus("Starting RNode Bluetooth pairing mode");
            port.write(RNodeUsbKissControl.bluetoothPairingModeFrame(), 1_000);

            final KissPinDecoder decoder = new KissPinDecoder();
            final byte[] buffer = new byte[256];
            final long deadline = System.currentTimeMillis() + PIN_WAIT_TIMEOUT_MS;
            while (!cancelled.get() && System.currentTimeMillis() < deadline) {
                final int read = port.read(buffer, READ_TIMEOUT_MS);
                if (read <= 0) {
                    continue;
                }
                final String pin = decoder.push(buffer, read);
                if (pin != null) {
                    listener.onPin(pin);
                    return new PairingModeResult(true, pin);
                }
            }
            listener.onStatus("RNode pairing mode started; waiting for manual PIN entry");
            return new PairingModeResult(true, null);
        } finally {
            closePort(port);
            activePort = null;
        }
    }

    void exitBluetoothPairingMode(int deviceId) throws IOException {
        final UsbSerialDriver driver = findDriver(deviceId);
        if (driver == null || driver.getPorts().isEmpty()) {
            return;
        }
        final UsbDevice device = driver.getDevice();
        if (!usbManager.hasPermission(device)) {
            return;
        }
        UsbSerialPort port = null;
        try {
            port = driver.getPorts().get(0);
            final android.hardware.usb.UsbDeviceConnection connection = usbManager.openDevice(device);
            if (connection == null) {
                return;
            }
            port.open(connection);
            port.setParameters(BAUD_RATE, UsbSerialPort.DATABITS_8, UsbSerialPort.STOPBITS_1, UsbSerialPort.PARITY_NONE);
            port.write(RNodeUsbKissControl.bluetoothDisableFrame(), 1_000);
            port.write(RNodeUsbKissControl.bluetoothEnableFrame(), 1_000);
        } finally {
            closePort(port);
        }
    }

    void cancel() {
        cancelled.set(true);
        closePort(activePort);
    }

    private List<UsbSerialDriver> findDrivers() {
        final List<UsbSerialDriver> drivers = new ArrayList<>(UsbSerialProber.getDefaultProber().findAllDrivers(usbManager));
        for (UsbSerialDriver driver : new UsbSerialProber(customProbeTable()).findAllDrivers(usbManager)) {
            if (!containsDeviceId(drivers, driver.getDevice().getDeviceId())) {
                drivers.add(driver);
            }
        }
        return drivers;
    }

    private UsbSerialDriver findDriver(int deviceId) {
        for (UsbSerialDriver driver : findDrivers()) {
            if (driver.getDevice().getDeviceId() == deviceId) {
                return driver;
            }
        }
        return null;
    }

    private UsbDevice findUsbDevice(int deviceId) {
        for (UsbDevice device : usbManager.getDeviceList().values()) {
            if (device.getDeviceId() == deviceId) {
                return device;
            }
        }
        return null;
    }

    private static boolean containsDeviceId(List<UsbSerialDriver> drivers, int deviceId) {
        for (UsbSerialDriver driver : drivers) {
            if (driver.getDevice().getDeviceId() == deviceId) {
                return true;
            }
        }
        return false;
    }

    private static ProbeTable customProbeTable() {
        final ProbeTable table = new ProbeTable();
        table.addProduct(0x0403, 0x6001, FtdiSerialDriver.class);
        table.addProduct(0x0403, 0x6010, FtdiSerialDriver.class);
        table.addProduct(0x0403, 0x6011, FtdiSerialDriver.class);
        table.addProduct(0x0403, 0x6014, FtdiSerialDriver.class);
        table.addProduct(0x0403, 0x6015, FtdiSerialDriver.class);
        table.addProduct(0x10C4, 0xEA60, Cp21xxSerialDriver.class);
        table.addProduct(0x10C4, 0xEA70, Cp21xxSerialDriver.class);
        table.addProduct(0x10C4, 0xEA71, Cp21xxSerialDriver.class);
        table.addProduct(0x067B, 0x2303, ProlificSerialDriver.class);
        table.addProduct(0x1A86, 0x5523, Ch34xSerialDriver.class);
        table.addProduct(0x1A86, 0x7523, Ch34xSerialDriver.class);
        table.addProduct(0x1A86, 0x55D4, Ch34xSerialDriver.class);
        table.addProduct(0x0483, 0x5740, CdcAcmSerialDriver.class);
        table.addProduct(0x2E8A, 0x0005, CdcAcmSerialDriver.class);
        table.addProduct(0x2E8A, 0x000A, CdcAcmSerialDriver.class);
        table.addProduct(0x239A, 0x8029, CdcAcmSerialDriver.class);
        table.addProduct(0x303A, 0x1001, CdcAcmSerialDriver.class);
        table.addProduct(0x303A, 0x4001, CdcAcmSerialDriver.class);
        return table;
    }

    private static void closePort(UsbSerialPort port) {
        if (port == null) {
            return;
        }
        try {
            port.close();
        } catch (IOException ex) {
            Log.w(TAG, "Failed to close USB serial port", ex);
        }
    }

    private static String safeManufacturerName(UsbDevice device) {
        try {
            return device.getManufacturerName();
        } catch (SecurityException ex) {
            return null;
        }
    }

    private static String safeProductName(UsbDevice device) {
        try {
            return device.getProductName();
        } catch (SecurityException ex) {
            return null;
        }
    }

    private static String safeSerialNumber(UsbDevice device) {
        try {
            return device.getSerialNumber();
        } catch (SecurityException ex) {
            return null;
        }
    }

    private static final class KissPinDecoder {
        private final List<Byte> payload = new ArrayList<>();
        private boolean inFrame;
        private boolean escape;
        private boolean hasCommand;
        private byte command;

        String push(byte[] bytes, int length) {
            for (int index = 0; index < length; index += 1) {
                final byte value = bytes[index];
                final String pin = push(value);
                if (pin != null) {
                    return pin;
                }
            }
            return null;
        }

        private String push(byte value) {
            if (value == RNodeUsbKissControl.FEND) {
                final String pin = finishFrame();
                inFrame = true;
                escape = false;
                hasCommand = false;
                payload.clear();
                return pin;
            }
            if (!inFrame) {
                return null;
            }
            final Byte decoded = decodeEscaped(value);
            if (decoded == null) {
                return null;
            }
            if (!hasCommand) {
                command = decoded;
                hasCommand = true;
                return null;
            }
            payload.add(decoded);
            return null;
        }

        private Byte decodeEscaped(byte value) {
            if (escape) {
                escape = false;
                if (value == RNodeUsbKissControl.TFEND) {
                    return RNodeUsbKissControl.FEND;
                }
                if (value == RNodeUsbKissControl.TFESC) {
                    return RNodeUsbKissControl.FESC;
                }
                return value;
            }
            if (value == RNodeUsbKissControl.FESC) {
                escape = true;
                return null;
            }
            return value;
        }

        private String finishFrame() {
            if (!inFrame || !hasCommand || command != RNodeUsbKissControl.CMD_BT_PIN || payload.size() < 4) {
                return null;
            }
            return RNodeUsbKissControl.decodeBluetoothPin(
                new byte[] { payload.get(0), payload.get(1), payload.get(2), payload.get(3) }
            );
        }
    }
}
