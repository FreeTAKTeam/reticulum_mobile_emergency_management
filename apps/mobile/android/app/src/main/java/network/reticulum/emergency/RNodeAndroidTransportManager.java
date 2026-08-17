package network.reticulum.emergency;

import android.Manifest;
import android.annotation.SuppressLint;
import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothGatt;
import android.bluetooth.BluetoothGattCallback;
import android.bluetooth.BluetoothGattCharacteristic;
import android.bluetooth.BluetoothGattDescriptor;
import android.bluetooth.BluetoothGattService;
import android.bluetooth.BluetoothManager;
import android.bluetooth.BluetoothProfile;
import android.content.Context;
import android.content.pm.PackageManager;
import android.os.Build;

import androidx.core.content.ContextCompat;

import org.json.JSONException;
import org.json.JSONObject;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.Arrays;
import java.util.UUID;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.atomic.AtomicReference;
import java.util.concurrent.locks.ReentrantLock;

/**
 * Service-owned, single-attempt Android byte transport for RNodes.
 *
 * Bluetooth calls are guarded by {@link #requireBluetoothReady()} before a
 * generation is installed. Cleanup also tolerates permission revocation.
 */
@SuppressLint("MissingPermission")
public final class RNodeAndroidTransportManager implements AutoCloseable {
    private static final UUID NUS_SERVICE = UUID.fromString("6e400001-b5a3-f393-e0a9-e50e24dcca9e");
    private static final UUID NUS_RX = UUID.fromString("6e400002-b5a3-f393-e0a9-e50e24dcca9e");
    private static final UUID NUS_TX = UUID.fromString("6e400003-b5a3-f393-e0a9-e50e24dcca9e");
    private static final UUID CCCD = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb");
    private static final UUID SPP = UUID.fromString("00001101-0000-1000-8000-00805f9b34fb");
    private static final int INBOUND_CAPACITY = 64;
    private static final int MAX_CHUNK_BYTES = 4 * 1024;
    private static final AtomicReference<RNodeAndroidTransportManager> INSTALLED =
        new AtomicReference<>();

    private final Context context;
    private final BluetoothAdapter adapter;
    private final AtomicLong latestGeneration = new AtomicLong();
    private final ExecutorService ioExecutor = Executors.newCachedThreadPool();
    private final Object sessionLock = new Object();
    private volatile Session current;

    public RNodeAndroidTransportManager(Context context) {
        this.context = context.getApplicationContext();
        final BluetoothManager manager =
            (BluetoothManager) this.context.getSystemService(Context.BLUETOOTH_SERVICE);
        this.adapter = manager == null ? null : manager.getAdapter();
    }

    public static void install(RNodeAndroidTransportManager manager) {
        final RNodeAndroidTransportManager previous = INSTALLED.getAndSet(manager);
        if (previous != null && previous != manager) {
            previous.close();
        }
    }

    public static void uninstall(RNodeAndroidTransportManager manager) {
        if (INSTALLED.compareAndSet(manager, null)) {
            manager.close();
        }
    }

    public static String open(long generation, String mode, String deviceId, long timeoutMs)
        throws Exception {
        return requireInstalled().openAttempt(generation, mode, deviceId, timeoutMs);
    }

    public static byte[] read(long generation, long timeoutMs) throws Exception {
        return requireInstalled().requireGeneration(generation).read(timeoutMs);
    }

    public static void write(long generation, byte[] payload, long timeoutMs) throws Exception {
        requireInstalled().requireGeneration(generation).write(payload, timeoutMs);
    }

    public static void close(long generation) {
        final RNodeAndroidTransportManager manager = INSTALLED.get();
        if (manager != null) {
            manager.closeGeneration(generation);
        }
    }

    private static RNodeAndroidTransportManager requireInstalled() {
        final RNodeAndroidTransportManager manager = INSTALLED.get();
        if (manager == null) {
            throw new IllegalStateException("RNode Android transport service is unavailable");
        }
        return manager;
    }

    private String openAttempt(long generation, String mode, String deviceId, long timeoutMs)
        throws Exception {
        requireBluetoothReady();
        if (deviceId == null || deviceId.trim().isEmpty()) {
            throw new IllegalArgumentException("RNode Bluetooth device id is required");
        }
        final long latest = latestGeneration.updateAndGet(previous -> Math.max(previous, generation));
        if (generation < latest) {
            throw new IOException("Stale RNode connection generation " + generation);
        }
        final BluetoothDevice device = adapter.getRemoteDevice(deviceId.trim());
        final Session session;
        if ("ble".equals(mode)) {
            session = new BleSession(generation, device);
        } else if ("bluetooth_classic".equals(mode)) {
            session = new SppSession(generation, device);
        } else {
            throw new IllegalArgumentException("Unsupported RNode Bluetooth mode: " + mode);
        }
        replaceCurrent(session);
        try {
            session.open(Math.max(1L, timeoutMs));
            if (!isCurrent(session)) {
                throw new IOException("RNode connection was superseded by a newer attempt");
            }
            return session.openResultJson();
        } catch (Exception error) {
            closeGeneration(generation);
            throw error;
        }
    }

    private void requireBluetoothReady() {
        if (adapter == null) {
            throw new IllegalStateException("Bluetooth is unavailable");
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S
            && ContextCompat.checkSelfPermission(context, Manifest.permission.BLUETOOTH_CONNECT)
                != PackageManager.PERMISSION_GRANTED) {
            throw new SecurityException("Bluetooth connect permission is required");
        }
        if (!adapter.isEnabled()) {
            throw new IllegalStateException("Bluetooth is disabled");
        }
    }

    private void replaceCurrent(Session next) {
        final Session previous;
        synchronized (sessionLock) {
            previous = current;
            current = next;
        }
        if (previous != null) {
            previous.close();
        }
    }

    private boolean isCurrent(Session session) {
        return current == session && !session.closed.get();
    }

    private Session requireGeneration(long generation) throws IOException {
        final Session session = current;
        if (session == null || session.generation != generation || session.closed.get()) {
            throw new IOException("Stale or closed RNode Android transport generation " + generation);
        }
        return session;
    }

    private void closeGeneration(long generation) {
        final Session session;
        synchronized (sessionLock) {
            if (current == null || current.generation != generation) {
                return;
            }
            session = current;
            current = null;
        }
        session.close();
    }

    @Override
    public void close() {
        final Session session;
        synchronized (sessionLock) {
            session = current;
            current = null;
        }
        if (session != null) {
            session.close();
        }
        ioExecutor.shutdownNow();
    }

    private abstract class Session implements AutoCloseable {
        final long generation;
        final LinkedBlockingQueue<ReadEvent> inbound =
            new LinkedBlockingQueue<>(INBOUND_CAPACITY);
        final AtomicBoolean closed = new AtomicBoolean(false);

        Session(long generation) {
            this.generation = generation;
        }

        abstract void open(long timeoutMs) throws Exception;

        abstract String mode();

        abstract Integer negotiatedMtu();

        abstract void write(byte[] payload, long timeoutMs) throws Exception;

        @Override
        public abstract void close();

        byte[] read(long timeoutMs) throws Exception {
            final ReadEvent event = inbound.poll(Math.max(1L, timeoutMs), TimeUnit.MILLISECONDS);
            if (event == null) {
                return null;
            }
            if (event.error != null) {
                throw new IOException(event.error);
            }
            return event.payload;
        }

        String openResultJson() throws JSONException {
            final JSONObject result = new JSONObject();
            result.put("generation", generation);
            result.put("kind", mode());
            final Integer mtu = negotiatedMtu();
            if (mtu == null) {
                result.put("negotiatedMtu", JSONObject.NULL);
            } else {
                result.put("negotiatedMtu", mtu);
            }
            return result.toString();
        }

        void offerBytes(byte[] payload) {
            if (payload == null || payload.length == 0 || payload.length > MAX_CHUNK_BYTES) {
                if (payload != null && payload.length > MAX_CHUNK_BYTES) {
                    fail("RNode inbound chunk exceeds 4096 bytes");
                }
                return;
            }
            if (!inbound.offer(ReadEvent.data(Arrays.copyOf(payload, payload.length)))) {
                fail("RNode inbound queue is full");
            }
        }

        void fail(String message) {
            close();
            inbound.clear();
            inbound.offer(ReadEvent.error(message));
        }
    }

    private final class BleSession extends Session {
        private final BluetoothDevice device;
        private final CountDownLatch connected = new CountDownLatch(1);
        private final CountDownLatch servicesReady = new CountDownLatch(1);
        private final CountDownLatch subscribed = new CountDownLatch(1);
        private final AtomicReference<String> setupError = new AtomicReference<>();
        private final ReentrantLock writeLock = new ReentrantLock();
        private final AtomicReference<CountDownLatch> pendingWrite = new AtomicReference<>();
        private final AtomicReference<String> writeError = new AtomicReference<>();
        private volatile BluetoothGatt gatt;
        private volatile BluetoothGattCharacteristic rx;
        private volatile int mtu = 23;

        BleSession(long generation, BluetoothDevice device) {
            super(generation);
            this.device = device;
        }

        @Override
        void open(long timeoutMs) throws Exception {
            final long deadline = System.nanoTime() + TimeUnit.MILLISECONDS.toNanos(timeoutMs);
            gatt = device.connectGatt(context, false, callback, BluetoothDevice.TRANSPORT_LE);
            if (gatt == null) {
                throw new IOException("Android did not create a Bluetooth GATT connection");
            }
            await(connected, deadline, "BLE connection");
            throwSetupError();
            if (!gatt.discoverServices()) {
                throw new IOException("Android did not start RNode GATT service discovery");
            }
            await(servicesReady, deadline, "BLE service discovery");
            throwSetupError();
            enableNotifications();
            await(subscribed, deadline, "BLE notification subscription");
            throwSetupError();
            // Data flow is ready at the default MTU. MTU negotiation is best effort.
            gatt.requestMtu(512);
        }

        @Override
        String mode() {
            return "ble";
        }

        @Override
        Integer negotiatedMtu() {
            return mtu;
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
                final int result;
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    result = activeGatt.writeCharacteristic(
                        characteristic,
                        payload,
                        BluetoothGattCharacteristic.WRITE_TYPE_DEFAULT
                    );
                } else {
                    characteristic.setWriteType(BluetoothGattCharacteristic.WRITE_TYPE_DEFAULT);
                    characteristic.setValue(payload);
                    result = activeGatt.writeCharacteristic(characteristic)
                        ? BluetoothGatt.GATT_SUCCESS
                        : BluetoothGatt.GATT_FAILURE;
                }
                if (result != BluetoothGatt.GATT_SUCCESS) {
                    throw new IOException("Android rejected RNode BLE write: " + result);
                }
                if (!completion.await(Math.max(1L, timeoutMs), TimeUnit.MILLISECONDS)) {
                    close();
                    throw new TimeoutException("RNode BLE write timed out");
                }
                final String error = writeError.get();
                if (error != null) {
                    throw new IOException(error);
                }
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
            if (isCurrent(this) && gatt == callbackGatt && !closed.get()) {
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
                if (status == BluetoothGatt.GATT_SUCCESS && newState == BluetoothProfile.STATE_CONNECTED) {
                    connected.countDown();
                    return;
                }
                final String error = "RNode BLE disconnected (status=" + status + ")";
                setupError.compareAndSet(null, error);
                connected.countDown();
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
            public void onDescriptorWrite(BluetoothGatt callbackGatt, BluetoothGattDescriptor descriptor, int status) {
                if (stale(callbackGatt) || !CCCD.equals(descriptor.getUuid())) {
                    return;
                }
                if (status != BluetoothGatt.GATT_SUCCESS) {
                    setupError.compareAndSet(null, "RNode BLE notification subscription failed: " + status);
                }
                subscribed.countDown();
            }

            @Override
            public void onCharacteristicChanged(BluetoothGatt callbackGatt, BluetoothGattCharacteristic characteristic) {
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
            public void onCharacteristicWrite(BluetoothGatt callbackGatt, BluetoothGattCharacteristic characteristic, int status) {
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
                if (!stale(callbackGatt) && status == BluetoothGatt.GATT_SUCCESS) {
                    mtu = value;
                }
            }
        };

        @Override
        public void close() {
            if (!closed.compareAndSet(false, true)) {
                return;
            }
            connected.countDown();
            servicesReady.countDown();
            subscribed.countDown();
            final CountDownLatch completion = pendingWrite.get();
            if (completion != null) {
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

    private final class SppSession extends Session {
        private final BluetoothDevice device;
        private final ReentrantLock writeLock = new ReentrantLock();
        private volatile android.bluetooth.BluetoothSocket socket;
        private volatile InputStream input;
        private volatile OutputStream output;

        SppSession(long generation, BluetoothDevice device) {
            super(generation);
            this.device = device;
        }

        @Override
        void open(long timeoutMs) throws Exception {
            adapter.cancelDiscovery();
            socket = device.createRfcommSocketToServiceRecord(SPP);
            final Future<?> connect = ioExecutor.submit(() -> {
                socket.connect();
                return null;
            });
            try {
                connect.get(timeoutMs, TimeUnit.MILLISECONDS);
            } catch (TimeoutException error) {
                close();
                connect.cancel(true);
                throw new TimeoutException("RNode Bluetooth Classic connection timed out");
            }
            input = socket.getInputStream();
            output = socket.getOutputStream();
            ioExecutor.execute(this::readLoop);
        }

        @Override
        String mode() {
            return "bluetooth_classic";
        }

        @Override
        Integer negotiatedMtu() {
            return null;
        }

        @Override
        void write(byte[] payload, long timeoutMs) throws Exception {
            if (payload == null || payload.length == 0 || payload.length > MAX_CHUNK_BYTES) {
                throw new IllegalArgumentException("RNode SPP write must contain 1..4096 bytes");
            }
            if (!writeLock.tryLock(Math.max(1L, timeoutMs), TimeUnit.MILLISECONDS)) {
                throw new TimeoutException("Timed out waiting for the RNode SPP write queue");
            }
            try {
                final Future<?> write = ioExecutor.submit(() -> {
                    final OutputStream activeOutput = output;
                    if (activeOutput == null || closed.get()) {
                        throw new IOException("RNode SPP connection is closed");
                    }
                    activeOutput.write(payload);
                    activeOutput.flush();
                    return null;
                });
                try {
                    write.get(Math.max(1L, timeoutMs), TimeUnit.MILLISECONDS);
                } catch (TimeoutException error) {
                    close();
                    write.cancel(true);
                    throw new TimeoutException("RNode SPP write timed out");
                }
            } finally {
                writeLock.unlock();
            }
        }

        private void readLoop() {
            final byte[] buffer = new byte[MAX_CHUNK_BYTES];
            try {
                while (!closed.get()) {
                    final int count = input.read(buffer);
                    if (count < 0) {
                        fail("RNode SPP stream closed by remote device");
                        return;
                    }
                    if (count > 0) {
                        offerBytes(Arrays.copyOf(buffer, count));
                    }
                }
            } catch (IOException error) {
                if (!closed.get()) {
                    fail("RNode SPP read failed: " + error.getMessage());
                }
            }
        }

        @Override
        public void close() {
            if (!closed.compareAndSet(false, true)) {
                return;
            }
            inbound.clear();
            inbound.offer(ReadEvent.error("RNode SPP connection closed"));
            final android.bluetooth.BluetoothSocket activeSocket = socket;
            socket = null;
            input = null;
            output = null;
            if (activeSocket != null) {
                try {
                    activeSocket.close();
                } catch (IOException ignored) {
                    // Closing is best effort and remains idempotent.
                }
            }
        }
    }

    private static final class ReadEvent {
        final byte[] payload;
        final String error;

        private ReadEvent(byte[] payload, String error) {
            this.payload = payload;
            this.error = error;
        }

        static ReadEvent data(byte[] payload) {
            return new ReadEvent(payload, null);
        }

        static ReadEvent error(String error) {
            return new ReadEvent(null, error);
        }
    }
}
