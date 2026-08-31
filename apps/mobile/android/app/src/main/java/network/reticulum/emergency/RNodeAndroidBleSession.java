package network.reticulum.emergency;

import android.annotation.SuppressLint;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothGatt;
import android.bluetooth.BluetoothGattCallback;
import android.bluetooth.BluetoothGattCharacteristic;
import android.bluetooth.BluetoothGattDescriptor;
import android.bluetooth.BluetoothGattService;
import android.bluetooth.BluetoothProfile;
import android.content.Context;
import android.os.Build;

import java.io.IOException;
import java.util.UUID;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.concurrent.atomic.AtomicReference;
import java.util.concurrent.locks.ReentrantLock;

/** One generation-scoped Nordic UART GATT attempt. */
@SuppressLint("MissingPermission")
final class RNodeAndroidBleSession extends RNodeAndroidSession {
    private static final int REQUESTED_ATT_MTU = 517;
    // LXMF/RNode notifications can carry 170 bytes. ATT reserves 3 bytes for
    // its header, so a smaller negotiated MTU cannot carry a complete frame.
    static final int MIN_LXMF_ATT_MTU = 173;
    private static final UUID NUS_SERVICE =
        UUID.fromString("6e400001-b5a3-f393-e0a9-e50e24dcca9e");
    private static final UUID NUS_RX =
        UUID.fromString("6e400002-b5a3-f393-e0a9-e50e24dcca9e");
    private static final UUID NUS_TX =
        UUID.fromString("6e400003-b5a3-f393-e0a9-e50e24dcca9e");
    private static final UUID CCCD =
        UUID.fromString("00002902-0000-1000-8000-00805f9b34fb");

    private final RNodeAndroidTransportManager manager;
    private final Context context;
    private final BluetoothDevice device;
    private final CountDownLatch connected = new CountDownLatch(1);
    private final CountDownLatch mtuReady = new CountDownLatch(1);
    private final CountDownLatch servicesReady = new CountDownLatch(1);
    private final CountDownLatch subscribed = new CountDownLatch(1);
    private final AtomicReference<String> setupError = new AtomicReference<>();
    private final ReentrantLock writeLock = new ReentrantLock();
    private final AtomicReference<CountDownLatch> pendingWrite = new AtomicReference<>();
    private final AtomicReference<String> writeError = new AtomicReference<>();
    private volatile BluetoothGatt gatt;
    private volatile BluetoothGattCharacteristic rx;
    private volatile int mtu = 23;

    RNodeAndroidBleSession(
        RNodeAndroidTransportManager manager,
        Context context,
        long generation,
        BluetoothDevice device
    ) {
        super(generation);
        this.manager = manager;
        this.context = context;
        this.device = device;
    }

    @Override
    void open(long timeoutMs) throws Exception {
        if (device.getBondState() != BluetoothDevice.BOND_BONDED) {
            throw new IOException(
                "RNode BLE is not paired. Put the RNode in pairing mode, pair it in REM, then retry."
            );
        }
        final long deadline = System.nanoTime() + TimeUnit.MILLISECONDS.toNanos(timeoutMs);
        gatt = device.connectGatt(context, false, callback, BluetoothDevice.TRANSPORT_LE);
        if (gatt == null) {
            throw new IOException("Android did not create a Bluetooth GATT connection");
        }
        await(connected, deadline, "BLE connection");
        throwSetupError();
        if (!gatt.requestMtu(REQUESTED_ATT_MTU)) {
            throw new IOException("Android did not start RNode BLE MTU negotiation");
        }
        await(mtuReady, deadline, "BLE MTU negotiation");
        throwSetupError();
        if (!isUsableAttMtu(mtu)) {
            throw new IOException(
                "RNode BLE negotiated ATT MTU is too small for LXMF: "
                    + mtu
                    + " (minimum "
                    + MIN_LXMF_ATT_MTU
                    + ")"
            );
        }
        if (!gatt.discoverServices()) {
            throw new IOException("Android did not start RNode GATT service discovery");
        }
        await(servicesReady, deadline, "BLE service discovery");
        throwSetupError();
        enableNotifications();
        await(subscribed, deadline, "BLE notification subscription");
        throwSetupError();
    }

    @Override
    String mode() {
        return "ble";
    }

    @Override
    Integer negotiatedMtu() {
        return mtu;
    }

    static boolean isUsableAttMtu(int negotiatedMtu) {
        return negotiatedMtu >= MIN_LXMF_ATT_MTU;
    }

    @Override
    void write(byte[] payload, long timeoutMs) throws Exception {
        if (payload == null || payload.length == 0 || payload.length > MAX_CHUNK_BYTES) {
            throw new IllegalArgumentException("RNode BLE write must contain 1..4096 bytes");
        }
        if (!writeLock.tryLock(Math.max(1L, timeoutMs), TimeUnit.MILLISECONDS)) {
            throw new TimeoutException("Timed out waiting for the RNode BLE write queue");
        }
        try {
            final BluetoothGatt activeGatt = gatt;
            final BluetoothGattCharacteristic characteristic = rx;
            if (closed.get() || activeGatt == null || characteristic == null) {
                throw new IOException("RNode BLE connection is closed");
            }
            final CountDownLatch completion = new CountDownLatch(1);
            pendingWrite.set(completion);
            writeError.set(null);
            final int writeType = BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE;
            final int result;
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                result = activeGatt.writeCharacteristic(
                    characteristic,
                    payload,
                    writeType
                );
            } else {
                // Nordic UART RX is the RNode command/data ingress. Match the
                // native btleplug bearer and use write-without-response so a
                // burst of KISS chunks does not serialize on callbacks that
                // are unrelated to the notification stream.
                characteristic.setWriteType(BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE);
                characteristic.setValue(payload);
                result = activeGatt.writeCharacteristic(characteristic)
                    ? BluetoothGatt.GATT_SUCCESS
                    : BluetoothGatt.GATT_FAILURE;
            }
            if (result != BluetoothGatt.GATT_SUCCESS) {
                throw new IOException("Android rejected RNode BLE write: " + result);
            }
            // WRITE_TYPE_NO_RESPONSE controls the remote ATT acknowledgement;
            // Android still serializes GATT operations and reports completion
            // through onCharacteristicWrite. Waiting for that callback keeps
            // the next KISS chunk from being rejected with
            // ERROR_GATT_WRITE_REQUEST_BUSY (201).
            if (!completion.await(Math.max(1L, timeoutMs), TimeUnit.MILLISECONDS)) {
                close();
                throw new TimeoutException("RNode BLE write timed out");
            }
            final String error = writeError.get();
            if (error != null) {
                throw new IOException(error);
            }
            if (closed.get()) {
                throw new IOException("RNode BLE connection closed during write");
            }
            recordWrite(payload);
        } finally {
            pendingWrite.set(null);
            writeLock.unlock();
        }
    }

    private void await(CountDownLatch latch, long deadline, String operation) throws Exception {
        final long remaining = deadline - System.nanoTime();
        if (remaining <= 0 || !latch.await(remaining, TimeUnit.NANOSECONDS)) {
            throw new TimeoutException(operation + " timed out");
        }
    }

    private void throwSetupError() throws IOException {
        final String error = setupError.get();
        if (error != null) {
            throw new IOException(error);
        }
    }

    private void enableNotifications() throws IOException {
        final BluetoothGattService service = gatt.getService(NUS_SERVICE);
        if (service == null) {
            throw new IOException("RNode Nordic UART service is missing");
        }
        rx = service.getCharacteristic(NUS_RX);
        final BluetoothGattCharacteristic tx = service.getCharacteristic(NUS_TX);
        if (rx == null || tx == null) {
            throw new IOException("RNode Nordic UART characteristics are missing");
        }
        final BluetoothGattDescriptor descriptor = tx.getDescriptor(CCCD);
        if (descriptor == null || !gatt.setCharacteristicNotification(tx, true)) {
            throw new IOException("Android could not enable RNode BLE notifications");
        }
        final int result;
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            result = gatt.writeDescriptor(descriptor, BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE);
        } else {
            descriptor.setValue(BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE);
            result = gatt.writeDescriptor(descriptor)
                ? BluetoothGatt.GATT_SUCCESS
                : BluetoothGatt.GATT_FAILURE;
        }
        if (result != BluetoothGatt.GATT_SUCCESS) {
            throw new IOException("Android rejected RNode BLE notification subscription: " + result);
        }
    }

    private boolean stale(BluetoothGatt callbackGatt) {
        if (manager.isCurrent(this) && gatt == callbackGatt && !closed.get()) {
            return false;
        }
        try {
            callbackGatt.disconnect();
        } catch (SecurityException ignored) {
            // Permission can be revoked while a stale callback is in flight.
        } finally {
            callbackGatt.close();
        }
        return true;
    }

    private final BluetoothGattCallback callback = new BluetoothGattCallback() {
        @Override
        public void onConnectionStateChange(BluetoothGatt callbackGatt, int status, int newState) {
            if (stale(callbackGatt)) {
                return;
            }
            if (status == BluetoothGatt.GATT_SUCCESS
                && newState == BluetoothProfile.STATE_CONNECTED) {
                connected.countDown();
                return;
            }
            final String error = "RNode BLE disconnected (status=" + status + ")";
            setupError.compareAndSet(null, error);
            connected.countDown();
            mtuReady.countDown();
            servicesReady.countDown();
            subscribed.countDown();
            fail(error);
        }

        @Override
        public void onServicesDiscovered(BluetoothGatt callbackGatt, int status) {
            if (stale(callbackGatt)) {
                return;
            }
            if (status != BluetoothGatt.GATT_SUCCESS) {
                setupError.compareAndSet(null, "RNode BLE service discovery failed: " + status);
            }
            servicesReady.countDown();
        }

        @Override
        public void onDescriptorWrite(
            BluetoothGatt callbackGatt,
            BluetoothGattDescriptor descriptor,
            int status
        ) {
            if (stale(callbackGatt) || !CCCD.equals(descriptor.getUuid())) {
                return;
            }
            if (status != BluetoothGatt.GATT_SUCCESS) {
                setupError.compareAndSet(
                    null,
                    "RNode BLE notification subscription failed: " + status
                );
            }
            subscribed.countDown();
        }

        @Override
        public void onCharacteristicChanged(
            BluetoothGatt callbackGatt,
            BluetoothGattCharacteristic characteristic
        ) {
            if (!stale(callbackGatt) && NUS_TX.equals(characteristic.getUuid())) {
                offerBytes(characteristic.getValue());
            }
        }

        @Override
        public void onCharacteristicChanged(
            BluetoothGatt callbackGatt,
            BluetoothGattCharacteristic characteristic,
            byte[] value
        ) {
            if (!stale(callbackGatt) && NUS_TX.equals(characteristic.getUuid())) {
                offerBytes(value);
            }
        }

        @Override
        public void onCharacteristicWrite(
            BluetoothGatt callbackGatt,
            BluetoothGattCharacteristic characteristic,
            int status
        ) {
            if (stale(callbackGatt) || !NUS_RX.equals(characteristic.getUuid())) {
                return;
            }
            if (status != BluetoothGatt.GATT_SUCCESS) {
                writeError.set("RNode BLE write failed: " + status);
            }
            final CountDownLatch completion = pendingWrite.get();
            if (completion != null) {
                completion.countDown();
            }
        }

        @Override
        public void onMtuChanged(BluetoothGatt callbackGatt, int value, int status) {
            if (stale(callbackGatt)) {
                return;
            }
            if (status == BluetoothGatt.GATT_SUCCESS) {
                mtu = value;
            } else {
                setupError.compareAndSet(null, "RNode BLE MTU negotiation failed: " + status);
            }
            mtuReady.countDown();
        }
    };

    @Override
    public void close() {
        if (!closed.compareAndSet(false, true)) {
            return;
        }
        connected.countDown();
        mtuReady.countDown();
        servicesReady.countDown();
        subscribed.countDown();
        final CountDownLatch completion = pendingWrite.get();
        if (completion != null) {
            writeError.compareAndSet(null, "RNode BLE connection closed during write");
            completion.countDown();
        }
        inbound.clear();
        inbound.offer(ReadEvent.error("RNode BLE connection closed"));
        final BluetoothGatt activeGatt = gatt;
        gatt = null;
        rx = null;
        if (activeGatt != null) {
            try {
                activeGatt.disconnect();
            } catch (SecurityException ignored) {
                // GATT close itself remains available after permission revocation.
            } finally {
                activeGatt.close();
            }
        }
    }
}
