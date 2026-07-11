package network.reticulum.emergency.plugins;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertTrue;

import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.ServiceConnection;
import android.os.IBinder;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import androidx.test.platform.app.InstrumentationRegistry;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import network.reticulum.emergency.plugin.api.IRemPluginHost;
import network.reticulum.emergency.plugin.api.IRemPluginService;
import network.reticulum.emergency.plugin.api.PluginProtocol;
import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;

@RunWith(AndroidJUnit4.class)
public final class PluginBinderIntegrationTest {
    @Test
    public void discoversBindsAndReceivesFixtureSensorRequest() throws Exception {
        final Context context = InstrumentationRegistry.getInstrumentation().getTargetContext();
        final JSONArray discovered = PluginDiscovery.discover(context);
        JSONObject fixture = null;
        for (int index = 0; index < discovered.length(); index++) {
            final JSONObject candidate = discovered.getJSONObject(index);
            if ("org.freetakteam.rem.plugin.fixture".equals(candidate.optString("pluginId"))) {
                fixture = candidate;
                break;
            }
        }
        assertNotNull("fixture plugin must be installed before instrumentation", fixture);
        final JSONObject fixtureDescriptor = fixture;
        assertEquals(PluginProtocol.API_MAJOR, fixtureDescriptor.optInt("apiMajor"));

        final CountDownLatch connected = new CountDownLatch(1);
        final CountDownLatch requestReceived = new CountDownLatch(1);
        final AtomicReference<IRemPluginService> serviceReference = new AtomicReference<>();
        final AtomicReference<JSONObject> requestReference = new AtomicReference<>();
        final ServiceConnection connection = new ServiceConnection() {
            @Override
            public void onServiceConnected(ComponentName name, IBinder binder) {
                serviceReference.set(IRemPluginService.Stub.asInterface(binder));
                connected.countDown();
            }

            @Override
            public void onServiceDisconnected(ComponentName name) {
                serviceReference.set(null);
            }
        };
        final Intent intent = new Intent(PluginProtocol.SERVICE_ACTION).setComponent(
            new ComponentName(
                fixtureDescriptor.getString("packageName"),
                fixtureDescriptor.getString("serviceClassName")
            )
        );
        assertTrue(context.bindService(intent, connection, Context.BIND_AUTO_CREATE));
        try {
            assertTrue("plugin service did not connect", connected.await(10, TimeUnit.SECONDS));
            final IRemPluginService service = serviceReference.get();
            assertNotNull(service);
            final JSONObject descriptor = new JSONObject(service.getDescriptorJson());
            assertEquals(
                fixtureDescriptor.getString("pluginId"),
                descriptor.getString("pluginId")
            );
            service.start(
                new IRemPluginHost.Stub() {
                    @Override
                    public void submitRequest(String requestJson) {
                        try {
                            requestReference.set(PluginProtocol.requireEnvelope(requestJson));
                            requestReceived.countDown();
                        } catch (Exception ignored) {
                        }
                    }
                },
                new JSONObject()
                    .put("protocolVersion", PluginProtocol.API_MAJOR)
                    .put("apiMajor", PluginProtocol.API_MAJOR)
                    .put("apiMinor", PluginProtocol.API_MINOR)
                    .put("sessionId", "instrumentation")
                    .put("hostPackage", context.getPackageName())
                    .toString()
            );
            assertTrue(
                "fixture did not publish a host request",
                requestReceived.await(10, TimeUnit.SECONDS)
            );
            assertEquals("sensor.publish", requestReference.get().getString("operation"));
            service.stop("instrumentation-complete");
        } finally {
            context.unbindService(connection);
        }
    }
}
