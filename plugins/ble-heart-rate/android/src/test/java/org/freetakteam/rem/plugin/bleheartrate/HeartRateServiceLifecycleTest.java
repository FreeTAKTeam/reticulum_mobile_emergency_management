package org.freetakteam.rem.plugin.bleheartrate;

import static org.junit.Assert.*;

import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothGatt;
import android.bluetooth.BluetoothGattCallback;
import android.content.Context;
import android.os.Handler;
import android.os.Looper;
import java.lang.reflect.Field;
import java.time.Duration;
import network.reticulum.emergency.plugin.api.IRemPluginHost;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.Robolectric;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;
import org.robolectric.annotation.Implementation;
import org.robolectric.annotation.Implements;
import org.robolectric.Shadows;

@RunWith(RobolectricTestRunner.class)
@Config(sdk = 28, manifest = Config.NONE, shadows = {HeartRateServiceLifecycleTest.TrackingGatt.class})
public final class HeartRateServiceLifecycleTest {
    @Implements(BluetoothGatt.class)
    public static class TrackingGatt {
        static int disconnected;
        static int closed;
        @Implementation protected void disconnect() { disconnected++; }
        @Implementation protected void close() { closed++; }
    }

    @Test
    public void destroyReleasesMonitoringAndCancelsCallbacksWithoutHostStop() throws Exception {
        verifyCleanup(false);
    }

    @Test
    public void hostStopThenDestroyIsIdempotent() throws Exception {
        verifyCleanup(true);
    }

    private void verifyCleanup(boolean stopFirst) throws Exception {
        final var controller = Robolectric.buildService(HeartRatePluginService.class).create();
        final HeartRatePluginService service = controller.get();
        TrackingGatt.disconnected = 0;
        TrackingGatt.closed = 0;
        set(service, "host", new IRemPluginHost.Stub() {
            @Override public void submitRequest(String requestJson) {}
        });
        set(service, "gatt", org.robolectric.util.ReflectionHelpers.callConstructor(BluetoothGatt.class));
        set(service, "foreground", true);
        final boolean[] fired = {false};
        final Handler handler = (Handler) field(service, "handler");
        final Runnable pending = () -> fired[0] = true;
        set(service, "pendingReconnect", pending);
        handler.postDelayed(pending, 1_000L);
        handler.postDelayed(() -> fired[0] = true, 2_000L);
        if (stopFirst) service.onPluginStop("test");
        controller.destroy();
        Shadows.shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMinutes(1));
        assertFalse(fired[0]);
        assertNull(field(service, "host"));
        assertNull(field(service, "gatt"));
        assertNull(field(service, "pendingReconnect"));
        assertNull(field(service, "instance"));
        assertEquals(false, field(service, "foreground"));
        assertEquals(1, TrackingGatt.disconnected);
        assertEquals(1, TrackingGatt.closed);
    }

    private static void set(Object object, String name, Object value) throws Exception {
        final Field field = HeartRatePluginService.class.getDeclaredField(name);
        field.setAccessible(true);
        field.set(object, value);
    }

    private static Object field(Object object, String name) throws Exception {
        final Field field = HeartRatePluginService.class.getDeclaredField(name);
        field.setAccessible(true);
        return field.get(object);
    }
}
