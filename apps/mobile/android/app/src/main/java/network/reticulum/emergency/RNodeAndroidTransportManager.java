package network.reticulum.emergency;

import android.Manifest;
import android.annotation.SuppressLint;
import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothManager;
import android.content.Context;
import android.content.pm.PackageManager;
import android.os.Build;
import android.util.Log;

import androidx.core.content.ContextCompat;

import java.io.IOException;
import org.json.JSONObject;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.atomic.AtomicReference;

/**
 * Service-owned, single-attempt Android byte transport for RNodes.
 *
 * Bluetooth calls are guarded by {@link #requireBluetoothReady()} before a
 * generation is installed. Cleanup also tolerates permission revocation.
 */
@SuppressLint("MissingPermission")
public final class RNodeAndroidTransportManager implements AutoCloseable {
    private static final String TAG = "RNodeAndroidTransport";
    private static final AtomicReference<RNodeAndroidTransportManager> INSTALLED =
        new AtomicReference<>();

    private final Context context;
    private final BluetoothAdapter adapter;
    private final AtomicLong latestGeneration = new AtomicLong();
    private final ExecutorService ioExecutor = Executors.newCachedThreadPool();
    private final Object sessionLock = new Object();
    private volatile RNodeAndroidSession current;

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

    public static String status() {
        final JSONObject result = new JSONObject();
        final RNodeAndroidTransportManager manager = INSTALLED.get();
        try {
            result.put("installed", manager != null);
            if (manager == null) {
                result.put("session", JSONObject.NULL);
                return result.toString();
            }
            final RNodeAndroidSession session = manager.current;
            result.put("latestGeneration", manager.latestGeneration.get());
            result.put(
                "session",
                session == null ? JSONObject.NULL : new JSONObject(session.statusJson())
            );
            return result.toString();
        } catch (Exception error) {
            return "{\"installed\":true,\"statusError\":\""
                + error.getClass().getSimpleName()
                + "\"}";
        }
    }

    static void resetRNodeForTest() throws Exception {
        final RNodeAndroidTransportManager manager = requireInstalled();
        final RNodeAndroidSession session = manager.current;
        if (session == null || session.closed.get()) {
            throw new IOException("RNode Android transport session is unavailable");
        }
        session.write(RNodeUsbKissControl.hardResetFrame(), 5_000L);
    }

    static void queryRNodeStatsForTest() throws Exception {
        final RNodeAndroidTransportManager manager = requireInstalled();
        final RNodeAndroidSession session = manager.current;
        if (session == null || session.closed.get()) {
            throw new IOException("RNode Android transport session is unavailable");
        }
        session.write(RNodeUsbKissControl.statRxFrame(), 5_000L);
        session.write(RNodeUsbKissControl.statTxFrame(), 5_000L);
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
        final RNodeAndroidSession session;
        if ("ble".equals(mode)) {
            session = new RNodeAndroidBleSession(this, context, generation, device);
        } else if ("bluetooth_classic".equals(mode)) {
            session = new RNodeAndroidSppSession(adapter, ioExecutor, generation, device);
        } else {
            throw new IllegalArgumentException("Unsupported RNode Bluetooth mode: " + mode);
        }
        replaceCurrent(session);
        try {
            session.open(Math.max(1L, timeoutMs));
            if (!isCurrent(session)) {
                throw new IOException("RNode connection was superseded by a newer attempt");
            }
            final String openResult = session.openResultJson();
            Log.i(
                TAG,
                "opened generation=" + generation
                    + " mode=" + mode
                    + " negotiatedMtu=" + session.negotiatedMtu()
            );
            return openResult;
        } catch (Exception error) {
            Log.w(
                TAG,
                "open failed generation=" + generation
                    + " mode=" + mode
                    + " reason=" + error.getMessage()
            );
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

    private void replaceCurrent(RNodeAndroidSession next) {
        final RNodeAndroidSession previous;
        synchronized (sessionLock) {
            previous = current;
            current = next;
        }
        if (previous != null) {
            previous.close();
        }
    }

    boolean isCurrent(RNodeAndroidSession session) {
        return current == session && !session.closed.get();
    }

    private RNodeAndroidSession requireGeneration(long generation) throws IOException {
        final RNodeAndroidSession session = current;
        if (session == null || session.generation != generation || session.closed.get()) {
            throw new IOException("Stale or closed RNode Android transport generation " + generation);
        }
        return session;
    }

    private void closeGeneration(long generation) {
        final RNodeAndroidSession session;
        synchronized (sessionLock) {
            if (current == null || current.generation != generation) {
                return;
            }
            session = current;
            current = null;
        }
        session.close();
        Log.i(TAG, "closed generation=" + generation + " status=" + session.statusJson());
    }

    @Override
    public void close() {
        final RNodeAndroidSession session;
        synchronized (sessionLock) {
            session = current;
            current = null;
        }
        if (session != null) {
            session.close();
        }
        ioExecutor.shutdownNow();
    }
}
