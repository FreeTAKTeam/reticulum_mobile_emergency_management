package org.freetakteam.rem.plugin.watchstatus;

import static org.junit.Assert.*;
import java.net.HttpURLConnection;
import java.net.ServerSocket;
import java.net.Socket;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.atomic.AtomicReference;
import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.Test;

public class RadarEndpointTest {
    @Test public void protectedRadarAndExistingStatusCoexistWithoutExposingDataToMissingTokens() throws Exception {
        final AtomicLong clock = new AtomicLong(100);
        final RadarAccess access = new RadarAccess(clock::get);
        final AtomicInteger hostReads = new AtomicInteger();
        try (HostRequestBroker broker = new HostRequestBroker()) {
            configureSource(broker, hostReads);
            final RadarProvider radar = new RadarProvider(broker, access, () -> RadarSnapshotTest.NOW, clock::get);
            final WatchStatusServer server = new WatchStatusServer();
            final int port = freePort();
            server.apply(true, port, () -> "{\"type\":\"rem.watch.status\"}", radar);
            try {
                assertEquals(403, get(port, "/northbound/watch/radar", null).status);
                access.renew();
                assertEquals(401, get(port, "/northbound/watch/radar", null).status);
                assertEquals(0, hostReads.get());
                final String token = access.configuration().getString("radarToken");
                final Reply success = get(port, "/northbound/watch/radar", token);
                assertEquals(200, success.status);
                final JSONObject snapshot = new JSONObject(success.body);
                assertEquals("request-1", snapshot.getString("requestId"));
                assertEquals(1, snapshot.getJSONArray("markers").length());
                assertFalse(success.body.contains(token));
                assertEquals(3, hostReads.get());
                assertEquals(429, get(port, "/northbound/watch/radar", token).status);
                final Reply status = get(port, "/info.json", null);
                assertEquals(200, status.status);
                assertFalse(status.body.contains("markers"));
                assertEquals("{\"ok\":true}", get(port, "/health", null).body);
                clock.addAndGet(RadarAccess.LIFETIME_MS);
                assertEquals(403, get(port, "/northbound/watch/radar", token).status);
                assertEquals(3, hostReads.get());
            } finally { server.stop(); }
        }
    }

    @Test public void permissionDenialRevokesSessionAndNeverReturnsCachedLocations() throws Exception {
        final AtomicLong clock = new AtomicLong(100);
        final RadarAccess access = new RadarAccess(clock::get);
        access.renew();
        final String token = access.configuration().getString("radarToken");
        try (HostRequestBroker broker = new HostRequestBroker()) {
            broker.start(json -> {
                final JSONObject request = new JSONObject(json);
                broker.accept(new JSONObject().put("protocolVersion", 1).put("requestId", request.getString("requestId")).put("ok", false)
                    .put("error", new JSONObject().put("code", "PermissionDenied")).toString());
            });
            final RadarProvider radar = new RadarProvider(broker, access, () -> RadarSnapshotTest.NOW, clock::get);
            final RadarFailure failure = assertThrows(RadarFailure.class, () -> radar.get("Bearer " + token, "r"));
            assertEquals("PermissionDenied", failure.code);
            assertFalse(access.active());
            assertThrows(RadarFailure.class, () -> radar.get("Bearer " + token, "r2"));
        }
    }

    @Test public void renewalDuringHostReadSuppressesLateSuccessAndFurtherReads() throws Exception {
        final RadarAccess access = new RadarAccess(() -> 100L);
        access.renew();
        final String token = access.configuration().getString("radarToken");
        final CountDownLatch submitted = new CountDownLatch(1);
        final AtomicReference<JSONObject> waiting = new AtomicReference<>();
        final java.util.concurrent.ExecutorService worker = Executors.newSingleThreadExecutor();
        try (HostRequestBroker broker = new HostRequestBroker()) {
            broker.start(json -> { waiting.set(new JSONObject(json)); submitted.countDown(); });
            final RadarProvider radar = new RadarProvider(broker, access, () -> RadarSnapshotTest.NOW, () -> 100L);
            final java.util.concurrent.Future<String> result = worker.submit(() -> radar.get("Bearer " + token, "r"));
            assertTrue(submitted.await(2, TimeUnit.SECONDS));
            access.renew();
            broker.accept(new JSONObject().put("protocolVersion", 1).put("requestId", waiting.get().getString("requestId")).put("ok", true)
                .put("result", RadarSnapshotTest.observer()).toString());
            final java.util.concurrent.ExecutionException failure = assertThrows(java.util.concurrent.ExecutionException.class,
                () -> result.get(2, TimeUnit.SECONDS));
            assertEquals("SessionExpired", ((RadarFailure) failure.getCause()).code);
            assertEquals(0, broker.pendingCount());
        } finally { worker.shutdownNow(); }
    }

    @Test public void oversizedAndSlowHeadersDoNotReachSourceAndStopClosesActiveConnections() throws Exception {
        final AtomicInteger reads = new AtomicInteger();
        final WatchStatusServer server = new WatchStatusServer();
        final int port = freePort();
        server.apply(true, port, () -> { reads.incrementAndGet(); return "{}"; });
        try {
            try (Socket client = new Socket("127.0.0.1", port)) {
                client.setSoTimeout(2_000);
                client.getOutputStream().write(("GET /info.json HTTP/1.1\r\nX-Large: " + "x".repeat(8_193) + "\r\n\r\n")
                    .getBytes(StandardCharsets.US_ASCII));
                assertClosed(client);
            }
            try (Socket client = new Socket("127.0.0.1", port)) {
                client.setSoTimeout(2_000);
                client.getOutputStream().write("GET /info.json HTTP/1.1\r\nX: ".getBytes(StandardCharsets.US_ASCII));
                assertClosed(client);
            }
            assertEquals(0, reads.get());
            assertEquals(200, get(port, "/health", null).status);
            try (Socket client = new Socket("127.0.0.1", port)) {
                server.stop();
                client.setSoTimeout(2_000);
                assertClosed(client);
            }
        } finally { server.stop(); }
    }

    private static void configureSource(HostRequestBroker broker, AtomicInteger reads) {
        broker.start(json -> {
            final JSONObject request = new JSONObject(json);
            reads.incrementAndGet();
            final JSONObject source = "map.markers.nearby".equals(request.getString("operation"))
                ? RadarSnapshotTest.nearby(new JSONArray().put(RadarSnapshotTest.marker("east", 0, .001)))
                : RadarSnapshotTest.observer();
            broker.accept(new JSONObject().put("protocolVersion", 1).put("requestId", request.getString("requestId")).put("ok", true)
                .put("result", source).toString());
        });
    }

    private static int freePort() throws Exception {
        try (ServerSocket probe = new ServerSocket(0)) { return probe.getLocalPort(); }
    }

    private static void assertClosed(Socket client) throws Exception {
        try { assertEquals(-1, client.getInputStream().read()); }
        catch (java.net.SocketException reset) {
            // Closing a socket with unread oversized headers may send TCP RST.
        }
    }

    private static final class Reply {
        int status;
        String body;
    }

    private static Reply get(int port, String path, String token) throws Exception {
        final HttpURLConnection connection = (HttpURLConnection) new URL("http://127.0.0.1:" + port + path).openConnection();
        connection.setConnectTimeout(2_000);
        connection.setReadTimeout(2_000);
        connection.setRequestProperty("X-REM-Radar-Request", "request-1");
        if (token != null) connection.setRequestProperty("Authorization", "Bearer " + token);
        try {
            final Reply reply = new Reply();
            reply.status = connection.getResponseCode();
            assertEquals("no-store", connection.getHeaderField("Cache-Control"));
            final java.io.InputStream input = reply.status >= 400 ? connection.getErrorStream() : connection.getInputStream();
            reply.body = new String(input.readAllBytes(), StandardCharsets.UTF_8);
            return reply;
        } finally { connection.disconnect(); }
    }
}
