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
    @Test public void samePersistedBearerReconnectsAfterSourceAndClockRestart() throws Exception {
        final java.nio.file.Path file = java.nio.file.Files.createTempFile("radar-test-pairing-", ".txt");
        final RadarAccess.Credentials saved = new RadarAccess.Credentials() {
            @Override public String read() {
                try { return new String(java.nio.file.Files.readAllBytes(file), StandardCharsets.US_ASCII); }
                catch (java.io.IOException error) { throw new IllegalStateException(error); }
            }
            @Override public void write(String token) {
                try { java.nio.file.Files.write(file, token.getBytes(StandardCharsets.US_ASCII)); }
                catch (java.io.IOException error) { throw new IllegalStateException(error); }
            }
        };
        final WatchStatusServer server = new WatchStatusServer();
        final int port = freePort();
        final AtomicInteger reads = new AtomicInteger();
        final AtomicLong clock = new AtomicLong(30L * 24 * 60 * 60 * 1000);
        try (HostRequestBroker broker = new HostRequestBroker()) {
            configureSource(broker, reads);
            final RadarAccess original = new RadarAccess(saved);
            original.resume();
            final String token = saved.read();
            final RadarAccess.Ticket oldTicket = original.authorize("Bearer " + token);
            server.apply(true, port, () -> "{}", new RadarProvider(broker, original, () -> RadarSnapshotTest.NOW, clock::get));
            final Reply before = get(port, "/northbound/watch/radar", token);
            assertEquals(200, before.status);
            original.pause();
            assertEquals(503, get(port, "/northbound/watch/radar", token).status);
            server.stop();
            broker.stop();
            assertThrows(RadarFailure.class, () -> original.requireCurrent(oldTicket));
            clock.set(0); // Phone reboot resets the elapsed clock, not pairing.
            final RadarAccess restarted = new RadarAccess(saved);
            restarted.resume();
            configureSource(broker, reads);
            server.apply(true, port, () -> "{}", new RadarProvider(broker, restarted, () -> RadarSnapshotTest.NOW, clock::get));
            final Reply after = get(port, "/northbound/watch/radar", token);
            assertEquals(200, after.status);
            assertEquals(6, reads.get());
            assertNotEquals(new JSONObject(before.body).getString("sessionId"), new JSONObject(after.body).getString("sessionId"));
            assertEquals(1, new JSONObject(after.body).getInt("sequence"));
            assertEquals(token, saved.read());
            assertFalse(after.body.contains(token));
            restarted.revoke();
            assertEquals("", saved.read());
        } finally {
            server.stop();
            java.nio.file.Files.deleteIfExists(file);
        }
    }

    @Test public void protectedRadarAndExistingStatusCoexistWithoutExposingDataToMissingTokens() throws Exception {
        final AtomicLong clock = new AtomicLong(100);
        final RadarAccess access = new RadarAccess(new RadarAccessAndBrokerTest.MemoryCredentials());
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
                clock.addAndGet(30L * 24 * 60 * 60 * 1000); // Pairing outlives the former eight-hour limit.
                assertEquals(200, get(port, "/northbound/watch/radar", token).status);
                access.revoke();
                assertEquals(403, get(port, "/northbound/watch/radar", token).status);
                assertEquals(6, hostReads.get());
            } finally { server.stop(); }
        }
    }

    @Test public void permissionDenialRevokesSessionAndNeverReturnsCachedLocations() throws Exception {
        final AtomicLong clock = new AtomicLong(100);
        final RadarAccessAndBrokerTest.MemoryCredentials saved = new RadarAccessAndBrokerTest.MemoryCredentials();
        final RadarAccess access = new RadarAccess(saved);
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
            assertEquals("", saved.token);
            final RadarAccess restarted = new RadarAccess(saved);
            restarted.resume();
            assertThrows(RadarFailure.class, () -> restarted.authorize("Bearer " + token));
            assertThrows(RadarFailure.class, () -> radar.get("Bearer " + token, "r2"));
        }
    }

    @Test public void replacementOrRestartDuringHostReadSuppressesLateSuccessAndRevocation() throws Exception {
        for (boolean restart : new boolean[] { false, true }) {
            lateHostReadCannotChangeReplacement(restart, false);
            lateHostReadCannotChangeReplacement(restart, true);
        }
    }

    private static void lateHostReadCannotChangeReplacement(boolean restart, boolean denied) throws Exception {
        final RadarAccess access = new RadarAccess(new RadarAccessAndBrokerTest.MemoryCredentials());
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
            if (restart) { access.pause(); access.resume(); }
            else access.renew();
            final String replacement = access.configuration().getString("radarToken");
            final JSONObject response = new JSONObject().put("protocolVersion", 1)
                .put("requestId", waiting.get().getString("requestId")).put("ok", !denied);
            if (denied) response.put("error", new JSONObject().put("code", "PermissionDenied"));
            else response.put("result", RadarSnapshotTest.observer());
            broker.accept(response.toString());
            final java.util.concurrent.ExecutionException failure = assertThrows(java.util.concurrent.ExecutionException.class,
                () -> result.get(2, TimeUnit.SECONDS));
            assertEquals(restart ? "HostUnavailable" : "SessionExpired",
                ((RadarFailure) failure.getCause()).code);
            assertTrue(access.active());
            assertEquals(replacement, access.configuration().getString("radarToken"));
            access.authorize("Bearer " + replacement);
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
