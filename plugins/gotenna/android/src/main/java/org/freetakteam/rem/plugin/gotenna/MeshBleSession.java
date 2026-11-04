package org.freetakteam.rem.plugin.gotenna;

import android.annotation.SuppressLint;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothGatt;
import android.bluetooth.BluetoothGattCallback;
import android.bluetooth.BluetoothGattCharacteristic;
import android.bluetooth.BluetoothGattDescriptor;
import android.bluetooth.BluetoothGattService;
import android.bluetooth.BluetoothProfile;
import android.content.Context;
import android.util.Log;
import java.io.IOException;
import java.util.Arrays;
import java.util.UUID;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.TimeUnit;

/** One owner, one worker and one outstanding GATT operation. Never retries a send. */
@SuppressLint("MissingPermission")
@SuppressWarnings("deprecation")
final class MeshBleSession implements AutoCloseable {
    static final UUID SERVICE = UUID.fromString("1276aaee-df5e-11e6-bf01-fe55135034f3");
    private static final UUID WRITE = UUID.fromString("1276b20a-df5e-11e6-bf01-fe55135034f3");
    private static final UUID READ = UUID.fromString("1276b20b-df5e-11e6-bf01-fe55135034f3");
    private static final UUID STATUS = UUID.fromString("12762b18-df5e-11e6-bf01-fe55135034f3");
    private static final UUID CCCD = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb");
    private final ArrayBlockingQueue<Integer> operations = new ArrayBlockingQueue<>(8);
    private final ArrayBlockingQueue<byte[]> replies = new ArrayBlockingQueue<>(8);
    private final MeshFraming decoder = new MeshFraming();
    private volatile BluetoothGatt gatt;
    private BluetoothGattCharacteristic write;
    private volatile boolean closed;
    private volatile boolean ready;
    private volatile String failure;
    private int sequence;

    void connect(Context context, BluetoothDevice device) throws Exception {
        gatt = device.connectGatt(context, false, callbacks, BluetoothDevice.TRANSPORT_LE);
        if (gatt == null) throw new IOException("Android could not open the Mesh radio");
        awaitOperation();
        if (!gatt.discoverServices()) throw new IOException("Service discovery rejected");
        awaitOperation();
        final BluetoothGattService service = gatt.getService(SERVICE);
        if (service == null) throw new IOException("Selected device is not a goTenna Mesh");
        write = service.getCharacteristic(WRITE);
        if (write == null) throw new IOException("Mesh write characteristic missing");
        final int properties = write.getProperties();
        write.setWriteType((properties & BluetoothGattCharacteristic.PROPERTY_WRITE) != 0
            ? BluetoothGattCharacteristic.WRITE_TYPE_DEFAULT : BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE);
        subscribe(service.getCharacteristic(READ), BluetoothGattDescriptor.ENABLE_INDICATION_VALUE);
        subscribe(service.getCharacteristic(STATUS), BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE);
        checkOpen();
        ready = true;
    }

    private void subscribe(BluetoothGattCharacteristic characteristic, byte[] mode) throws Exception {
        if (characteristic == null || !gatt.setCharacteristicNotification(characteristic, true)) {
            throw new IOException("Mesh notifications unavailable");
        }
        final BluetoothGattDescriptor descriptor = characteristic.getDescriptor(CCCD);
        if (descriptor == null) throw new IOException("Mesh notification descriptor missing");
        descriptor.setValue(mode);
        if (!gatt.writeDescriptor(descriptor)) throw new IOException("Mesh subscription rejected");
        awaitOperation();
    }

    String failureMessage() { return failure == null ? "Mesh disconnected" : failure; }

    boolean isReady() { return ready && !closed && failure == null; }

    byte[] command(int opcode, byte[] payload) throws Exception {
        checkOpen();
        if (!isReady()) throw new IOException("Connect the Mesh radio first");
        sequence = (sequence + 1) & 255;
        if (sequence == 0x10) sequence++;
        replies.clear();
        final byte[] wire = MeshFraming.encode(opcode, sequence, payload);
        try {
            for (int offset = 0; offset < wire.length; offset += 20) {
                checkOpen();
                write.setValue(Arrays.copyOfRange(wire, offset, Math.min(wire.length, offset + 20)));
                if (!gatt.writeCharacteristic(write)) throw new IOException("Mesh write rejected");
                awaitOperation();
            }
            final long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(8);
            while (System.nanoTime() < deadline) {
                checkOpen();
                final byte[] reply = replies.poll(Math.max(1, deadline - System.nanoTime()), TimeUnit.NANOSECONDS);
                if (reply == null) break;
                checkOpen();
                if (reply.length < 2 || (reply[1] & 255) != sequence) continue;
                final int status = (reply[0] & 255) ^ opcode;
                if (opcode == 6 && status == 0xc0 && reply.length == 3 && reply[2] == (byte) 0xff) return null;
                if (status != 0x40) throw new IOException("Mesh command rejected (" + status + ")");
                return Arrays.copyOfRange(reply, 2, reply.length);
            }
            throw new IOException("Mesh response timed out; transmission outcome unknown. Reconnect before another action.");
        } catch (Exception error) {
            close();
            throw error;
        }
    }

    private void checkOpen() throws IOException {
        if (closed || failure != null) throw new IOException(failure == null ? "Mesh disconnected" : failure);
    }

    private void awaitOperation() throws Exception {
        checkOpen();
        final Integer status = operations.poll(10, TimeUnit.SECONDS);
        checkOpen();
        if (status == null || status != BluetoothGatt.GATT_SUCCESS) {
            throw new IOException("Mesh GATT operation failed or timed out");
        }
    }

    private final BluetoothGattCallback callbacks = new BluetoothGattCallback() {
        @Override public void onConnectionStateChange(BluetoothGatt connection, int status, int state) {
            if (closed) return;
            if (status == BluetoothGatt.GATT_SUCCESS && state == BluetoothProfile.STATE_CONNECTED) {
                operations.offer(status);
            } else if (state == BluetoothProfile.STATE_DISCONNECTED || status != BluetoothGatt.GATT_SUCCESS) {
                fail("Mesh connection ended (" + status + ")");
            }
        }
        @Override public void onServicesDiscovered(BluetoothGatt connection, int status) { operations.offer(status); }
        @Override public void onDescriptorWrite(BluetoothGatt connection, BluetoothGattDescriptor descriptor, int status) { operations.offer(status); }
        @Override public void onCharacteristicWrite(BluetoothGatt connection, BluetoothGattCharacteristic characteristic, int status) { operations.offer(status); }
        @Override public void onCharacteristicChanged(BluetoothGatt connection, BluetoothGattCharacteristic characteristic) {
            received(characteristic, characteristic.getValue());
        }
        @Override public void onCharacteristicChanged(BluetoothGatt connection, BluetoothGattCharacteristic characteristic, byte[] value) {
            received(characteristic, value);
        }
    };

    private void received(BluetoothGattCharacteristic characteristic, byte[] value) {
        if (closed || !READ.equals(characteristic.getUuid()) || value == null) return;
        try {
            for (byte[] frame : decoder.receive(value)) {
                if (!replies.offer(frame)) throw new IOException("Mesh response queue full");
            }
        } catch (Exception error) {
            fail(error.getMessage());
        }
    }

    private void fail(String message) {
        failure = message;
        ready = false;
        operations.offer(-1);
        replies.offer(new byte[0]);
    }

    @Override public synchronized void close() {
        closed = true;
        ready = false;
        operations.offer(-1);
        replies.offer(new byte[0]);
        final BluetoothGatt connection = gatt;
        gatt = null;
        if (connection != null) {
            try { connection.disconnect(); } catch (Exception error) { Log.w("REM.goTenna", "Disconnect failed", error); }
            try { connection.close(); } catch (Exception error) { Log.w("REM.goTenna", "GATT close failed", error); }
        }
    }
}
