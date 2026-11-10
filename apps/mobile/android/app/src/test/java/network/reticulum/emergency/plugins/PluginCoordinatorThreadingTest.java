package network.reticulum.emergency.plugins;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import android.content.ServiceConnection;
import android.os.Looper;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import network.reticulum.emergency.ReticulumBridge;
import network.reticulum.emergency.plugin.api.IRemPluginConfigurationCallback;
import network.reticulum.emergency.plugin.api.IRemPluginHost;
import network.reticulum.emergency.plugin.api.IRemPluginService;
import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.RuntimeEnvironment;
import org.robolectric.Shadows;
import org.robolectric.annotation.Config;
import org.robolectric.annotation.Implementation;
import org.robolectric.annotation.Implements;
import org.robolectric.annotation.LooperMode;

@RunWith(RobolectricTestRunner.class)
@Config(sdk = 28, manifest = Config.NONE, shadows = PluginCoordinatorThreadingTest.BridgeShadow.class)
@LooperMode(LooperMode.Mode.PAUSED)
public final class PluginCoordinatorThreadingTest {
    @Test
    public void blockedDescriptorDoesNotBlockMainAndCannotRestartDisabledPlugin() throws Exception {
        checkBlockedLifecycle(false);
    }

    @Test
    public void blockedStartDoesNotBlockMainOrPublishRunningAfterDisable() throws Exception {
        checkBlockedLifecycle(true);
    }

    private void checkBlockedLifecycle(boolean blockStart) throws Exception {
        BridgeShadow.running.set(false);
        final PluginCoordinator coordinator = new PluginCoordinator(RuntimeEnvironment.getApplication());
        final Field running = PluginCoordinator.class.getDeclaredField("nodeRunning");
        running.setAccessible(true);
        running.set(coordinator, true);
        final Class<?> type = Class.forName(PluginCoordinator.class.getName() + "$PluginConnection");
        final Constructor<?> constructor = type.getDeclaredConstructor(PluginCoordinator.class, JSONObject.class);
        constructor.setAccessible(true);
        final Object connection = constructor.newInstance(coordinator, new JSONObject()
            .put("pluginId", "test.plugin")
            .put("packageName", "test.plugin")
            .put("serviceClassName", "test.plugin.Service")
            .put("apiMajor", 1).put("apiMinor", 0));
        final BlockingPlugin service = new BlockingPlugin(blockStart);
        try {
            ((ServiceConnection) connection).onServiceConnected(null, service);
            assertTrue(service.entered.await(5, TimeUnit.SECONDS));
            assertFalse(service.descriptorOnMain.get());
            final AtomicBoolean mainResponded = new AtomicBoolean();
            new android.os.Handler(Looper.getMainLooper()).post(() -> mainResponded.set(true));
            Shadows.shadowOf(Looper.getMainLooper()).idle();
            assertTrue(mainResponded.get());
            final Method stop = type.getDeclaredMethod("stop", String.class);
            stop.setAccessible(true);
            stop.invoke(connection, "disabled");
            service.release.countDown();
            assertTrue(service.stopped.await(5, TimeUnit.SECONDS));
            Shadows.shadowOf(Looper.getMainLooper()).idle();
            org.junit.Assert.assertEquals(blockStart, service.started.get());
            assertFalse(service.stopOnMain.get());
            assertFalse(service.startOnMain.get());
            assertFalse(BridgeShadow.running.get());
        } finally {
            service.release.countDown();
            coordinator.close();
        }
    }

    private static final class BlockingPlugin extends IRemPluginService.Stub {
        final CountDownLatch entered = new CountDownLatch(1);
        final CountDownLatch release = new CountDownLatch(1);
        final CountDownLatch stopped = new CountDownLatch(1);
        final AtomicBoolean descriptorOnMain = new AtomicBoolean();
        final AtomicBoolean stopOnMain = new AtomicBoolean();
        final AtomicBoolean startOnMain = new AtomicBoolean();
        final AtomicBoolean started = new AtomicBoolean();

        final boolean blockStart;
        BlockingPlugin(boolean blockStart) { this.blockStart = blockStart; }

        private void block() {
            entered.countDown();
            try { release.await(5, TimeUnit.SECONDS); }
            catch (InterruptedException error) { Thread.currentThread().interrupt(); }
        }
        @Override public String getDescriptorJson() {
            descriptorOnMain.set(Looper.myLooper() == Looper.getMainLooper());
            if (!blockStart) { block(); }
            return "{\"pluginId\":\"test.plugin\",\"apiMajor\":1,\"apiMinor\":0}";
        }
        @Override public void start(IRemPluginHost host, String session) {
            startOnMain.set(Looper.myLooper() == Looper.getMainLooper());
            started.set(true);
            if (blockStart) { block(); }
        }
        @Override public void stop(String reason) {
            stopOnMain.set(Looper.myLooper() == Looper.getMainLooper());
            stopped.countDown();
        }
        @Override public void onHostEvent(String event) {}
        @Override public void onHostResponse(String response) {}
        @Override public void handleConfigurationRequest(String request, IRemPluginConfigurationCallback callback) {}
    }

    @Implements(value = ReticulumBridge.class, isInAndroidSdk = false)
    public static class BridgeShadow {
        static final AtomicBoolean running = new AtomicBoolean();
        @Implementation protected static int setPluginRuntimeStateJson(String json) throws Exception {
            running.set("Running".equals(new JSONObject(json).optString("state")));
            return 0;
        }
        @Implementation protected static String stopPluginMapSessionsJson(String json) { return "{}"; }
    }
}
