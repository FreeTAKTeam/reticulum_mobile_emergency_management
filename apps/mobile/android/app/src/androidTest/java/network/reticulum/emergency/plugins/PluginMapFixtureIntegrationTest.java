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
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import network.reticulum.emergency.plugin.api.IRemPluginHost;
import network.reticulum.emergency.plugin.api.IRemPluginService;
import network.reticulum.emergency.plugin.api.PluginProtocol;
import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;

/** Drives the map fixture APK across a real Binder boundary through the API 1.3 map flow. */
@RunWith(AndroidJUnit4.class)
public final class PluginMapFixtureIntegrationTest {
    private static final String PLUGIN_ID = "org.freetakteam.rem.plugin.map_fixture";
    private static final String SELECTED = "0123456789abcdef0123456789abcdef";

    private static JSONObject next(BlockingQueue<JSONObject> requests, String operation) throws Exception {
        final JSONObject request = requests.poll(10, TimeUnit.SECONDS);
        assertNotNull("fixture did not send " + operation, request);
        assertEquals(operation, request.getString("operation"));
        return request;
    }

    private static String ok(JSONObject request, JSONObject result) throws Exception {
        return new JSONObject()
            .put("protocolVersion", 1)
            .put("requestId", request.getString("requestId"))
            .put("ok", true)
            .put("result", result)
            .toString();
    }

    @Test
    public void fixtureCreatesAMarkerAndReadsTheSelectedSession() throws Exception {
        final Context context = InstrumentationRegistry.getInstrumentation().getTargetContext();
        final JSONArray discovered = PluginDiscovery.discover(context);
        JSONObject fixture = null;
        for (int index = 0; index < discovered.length(); index++) {
            if (PLUGIN_ID.equals(discovered.getJSONObject(index).optString("pluginId"))) {
                fixture = discovered.getJSONObject(index);
            }
        }
        assertNotNull("map fixture plugin must be installed before instrumentation", fixture);
        assertEquals(PluginProtocol.MAP_SESSION_API_MINOR, fixture.optInt("apiMinor"));
        assertTrue(fixture.getJSONObject("declaredCapabilities").optBoolean("mapSelectionRead"));

        final CountDownLatch connected = new CountDownLatch(1);
        final AtomicReference<IRemPluginService> serviceReference = new AtomicReference<>();
        final BlockingQueue<JSONObject> requests = new LinkedBlockingQueue<>();
        final ServiceConnection connection = new ServiceConnection() {
            @Override public void onServiceConnected(ComponentName name, IBinder binder) {
                serviceReference.set(IRemPluginService.Stub.asInterface(binder));
                connected.countDown();
            }
            @Override public void onServiceDisconnected(ComponentName name) { serviceReference.set(null); }
        };
        final Intent intent = new Intent(PluginProtocol.SERVICE_ACTION).setComponent(
            new ComponentName(fixture.getString("packageName"), fixture.getString("serviceClassName"))
        );
        assertTrue(context.bindService(intent, connection, Context.BIND_AUTO_CREATE));
        try {
            assertTrue(connected.await(10, TimeUnit.SECONDS));
            final IRemPluginService service = serviceReference.get();
            service.start(new IRemPluginHost.Stub() {
                @Override public void submitRequest(String requestJson) {
                    try {
                        requests.add(PluginProtocol.requireEnvelope(requestJson));
                    } catch (Exception ignored) {}
                }
            }, new JSONObject()
                .put("protocolVersion", 1)
                .put("apiMajor", 1)
                .put("apiMinor", PluginProtocol.API_MINOR)
                .put("sessionId", "map-instrumentation")
                .put("hostPackage", context.getPackageName())
                .toString());

            final JSONObject symbols = next(requests, "map.symbols.list");
            service.onHostResponse(ok(symbols, new JSONObject().put(
                "items", new JSONArray().put(new JSONObject().put("id", "hostile"))
            )));
            final JSONObject observer = next(requests, "map.observer.snapshot");
            service.onHostResponse(ok(observer, new JSONObject()
                .put("available", true)
                .put("observer", new JSONObject().put("kind", "localDevice"))
                .put("lat", 45.0)
                .put("lon", 7.5)
                .put("updatedAtMs", 1L)));
            final JSONObject create = next(requests, "map.markers.create");
            assertEquals("hostile", create.getJSONObject("payload").getString("symbol"));

            service.onHostEvent(PluginProtocol.hostEvent(
                PluginProtocol.EVENT_MAP_SESSION_STARTED,
                new JSONObject()
                    .put("sessionId", "s-1")
                    .put("pluginId", PLUGIN_ID)
                    .put("objectDestinationHashes", new JSONArray().put(SELECTED))
            ));
            final JSONObject read = next(requests, "map.session.objects.get");
            assertEquals("s-1", read.getJSONObject("payload").getString("sessionId"));
            service.stop("instrumentation-complete");
        } finally {
            context.unbindService(connection);
        }
    }
}
