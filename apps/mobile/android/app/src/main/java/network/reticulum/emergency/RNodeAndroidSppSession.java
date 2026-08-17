package network.reticulum.emergency;

import android.annotation.SuppressLint;
import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothSocket;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.Arrays;
import java.util.UUID;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.concurrent.locks.ReentrantLock;

/** One generation-scoped Bluetooth Classic SPP attempt. */
@SuppressLint("MissingPermission")
final class RNodeAndroidSppSession extends RNodeAndroidSession {
    private static final UUID SPP =
        UUID.fromString("00001101-0000-1000-8000-00805f9b34fb");

    private final BluetoothAdapter adapter;
    private final ExecutorService ioExecutor;
    private final BluetoothDevice device;
    private final ReentrantLock writeLock = new ReentrantLock();
    private volatile BluetoothSocket socket;
    private volatile InputStream input;
    private volatile OutputStream output;

    RNodeAndroidSppSession(
        BluetoothAdapter adapter,
        ExecutorService ioExecutor,
        long generation,
        BluetoothDevice device
    ) {
        super(generation);
        this.adapter = adapter;
        this.ioExecutor = ioExecutor;
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
        final BluetoothSocket activeSocket = socket;
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
