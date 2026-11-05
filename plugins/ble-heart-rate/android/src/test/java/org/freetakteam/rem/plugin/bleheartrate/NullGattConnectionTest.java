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
@Config(sdk = 28, manifest = Config.NONE, shadows = {NullGattConnectionTest.NullGattDevice.class})
public final class NullGattConnectionTest {
    @Implements(BluetoothDevice.class)
    public static class NullGattDevice {
        static int attempts;

        @Implementation
        protected BluetoothGatt connectGatt(Context context, boolean autoConnect,
            BluetoothGattCallback callback, int transport) {
            attempts++;
            return null;
        }
    }

    @Test
    public void nullGattStopsForegroundAndRetriesWithBoundedBackoff() throws Exception {
        final var controller = Robolectric.buildService(HeartRatePluginService.class).create();
        final HeartRatePluginService service = controller.get();
        new PluginPreferences(service).setDevice("00:11:22:33:44:55", "Sensor");
        Shadows.shadowOf(BluetoothAdapter.getDefaultAdapter()).setEnabled(true);
        NullGattDevice.attempts = 0;
        service.onPluginStart(new IRemPluginHost.Stub() {
            @Override public void submitRequest(String requestJson) {}
        }, "{}");
        assertEquals("Connection failed", field(service, "connectionState"));
        assertEquals(false, field(service, "foreground"));
        assertNotNull(field(service, "pendingReconnect"));
        Shadows.shadowOf(Looper.getMainLooper()).idleFor(Duration.ofSeconds(2));
        assertEquals(2, NullGattDevice.attempts);
        Shadows.shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMinutes(3));
        assertEquals(6, NullGattDevice.attempts);
        assertNull(field(service, "pendingReconnect"));
        assertEquals("Connection failed", field(service, "connectionState"));
        controller.destroy();
    }

    private static Object field(Object object, String name) throws Exception {
        final Field field = HeartRatePluginService.class.getDeclaredField(name);
        field.setAccessible(true);
        return field.get(object);
    }
}
