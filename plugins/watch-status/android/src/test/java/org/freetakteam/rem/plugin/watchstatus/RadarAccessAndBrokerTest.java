package org.freetakteam.rem.plugin.watchstatus;

import static org.junit.Assert.*;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;
import org.json.JSONObject;
import org.junit.Test;

public class RadarAccessAndBrokerTest {
    static final class MemoryCredentials implements RadarAccess.Credentials {
        String token = "";
        @Override public String read() { return token; }
        @Override public void write(String value) { token = value; }
    }

    @Test public void pairingPersistsAcrossAccessRecreationAndSuspendsOnlyRuntimeTickets() throws Exception {
        final MemoryCredentials saved = new MemoryCredentials();
        final RadarAccess access = new RadarAccess(saved);
        assertFalse(access.active());
        assertThrows(RadarFailure.class, () -> access.authorize(null));
        access.resume();
        final String token = access.configuration().getString("radarToken");
        assertEquals(64, token.length());
        assertThrows(RadarFailure.class, () -> access.authorize("Bearer wrong"));
        final RadarAccess.Ticket ticket = access.authorize("Bearer " + token);
        assertNotEquals(token, ticket.sessionId);
        access.resume(); // Ordinary settings saves do not change the active source session.
        access.requireCurrent(ticket);
        assertEquals(ticket.sessionId, access.authorize("Bearer " + token).sessionId);
        access.pause(); // Server/host stops pause access, not pairing.
        assertThrows(RadarFailure.class, () -> access.requireCurrent(ticket));
        assertThrows(RadarFailure.class, () -> access.authorize("Bearer " + token));
        assertTrue(access.configuration().getBoolean("radarPairingPersistent"));
        access.resume();
        assertNotEquals(ticket.sessionId, access.authorize("Bearer " + token).sessionId);
        final RadarAccess restarted = new RadarAccess(saved);
        assertFalse(restarted.active());
        restarted.resume();
        assertEquals(token, restarted.configuration().getString("radarToken"));
        assertNotEquals(ticket.sessionId, restarted.authorize("Bearer " + token).sessionId);
        assertTrue(restarted.configuration().getBoolean("radarPairingPersistent"));
    }

    @Test public void explicitReplacementAndRevocationPersistAndInvalidateTickets() throws Exception {
        final MemoryCredentials saved = new MemoryCredentials();
        final RadarAccess access = new RadarAccess(saved);
        access.resume();
        final String token = saved.token;
        final RadarAccess.Ticket ticket = access.authorize("Bearer " + token);
        access.renew();
        assertThrows(RadarFailure.class, () -> access.requireCurrent(ticket));
        assertThrows(RadarFailure.class, () -> access.authorize("Bearer " + token));
        final RadarAccess restarted = new RadarAccess(saved);
        restarted.resume();
        assertThrows(RadarFailure.class, () -> restarted.authorize("Bearer " + token));
        final String replacement = saved.token;
        access.revoke();
        assertFalse(access.active());
        assertEquals("", saved.token);
        assertEquals("", access.configuration().getString("radarToken"));
        final RadarAccess afterForget = new RadarAccess(saved);
        afterForget.resume();
        assertThrows(RadarFailure.class, () -> afterForget.authorize("Bearer " + replacement));
    }

    @Test public void corruptStorageIsReplacedAndWriteFailureCannotExposeUnpersistedPairing() throws Exception {
        final MemoryCredentials saved = new MemoryCredentials();
        saved.token = "corrupt";
        final RadarAccess access = new RadarAccess(saved);
        access.resume();
        assertTrue(saved.token.matches("[a-f0-9]{64}"));
        final RadarAccess failed = new RadarAccess(new RadarAccess.Credentials() {
            @Override public String read() { return ""; }
            @Override public void write(String token) { throw new IllegalStateException("Write failed"); }
        });
        assertThrows(IllegalStateException.class, failed::resume);
        assertFalse(failed.active());
    }

    @Test public void simultaneousOperationalAndRadarRepliesAreCorrelatedAndLateRepliesIgnored() throws Exception {
        try (HostRequestBroker broker = new HostRequestBroker()) {
            final List<JSONObject> sent = new ArrayList<>();
            broker.start(json -> sent.add(new JSONObject(json)));
            final CompletableFuture<JSONObject> status = broker.request("operational.snapshot", new JSONObject());
            final CompletableFuture<JSONObject> radar = broker.request("map.observer.snapshot", new JSONObject());
            broker.accept(new JSONObject().put("requestId", "wrong").put("ok", true).put("result", new JSONObject()).toString());
            assertFalse(status.isDone());
            broker.accept(reply(sent.get(1), "radar"));
            assertEquals("radar", radar.get().getString("value"));
            assertFalse(status.isDone());
            broker.stop();
            assertTrue(status.isCompletedExceptionally());
            broker.start(json -> sent.add(new JSONObject(json)));
            final CompletableFuture<JSONObject> fresh = broker.request("operational.snapshot", new JSONObject());
            broker.accept(reply(sent.get(0), "old"));
            assertFalse(fresh.isDone());
            broker.accept(reply(sent.get(2), "fresh"));
            assertEquals("fresh", fresh.get().getString("value"));
            assertEquals(0, broker.pendingCount());
        }
    }

    @Test public void malformedMissingOversizeAndLostRepliesReleaseSlotsOnFixedDeadline() throws Exception {
        try (HostRequestBroker broker = new HostRequestBroker(50)) {
            broker.start(json -> {});
            final List<CompletableFuture<JSONObject>> waiting = new ArrayList<>();
            for (int i = 0; i < 4; i++) waiting.add(broker.request("test", new JSONObject()));
            assertTrue(broker.request("overflow", new JSONObject()).isCompletedExceptionally());
            assertEquals(4, broker.pendingCount());
            broker.accept("{");
            broker.accept("x".repeat(65_537));
            broker.accept("{\"ok\":true,\"result\":{}}");
            for (CompletableFuture<JSONObject> future : waiting) {
                assertThrows(java.util.concurrent.ExecutionException.class, () -> future.get(2, TimeUnit.SECONDS));
            }
            assertEquals(0, broker.pendingCount());
            assertTrue(broker.request("oversized", new JSONObject().put("data", "x".repeat(65_536))).isCompletedExceptionally());
            assertEquals(0, broker.pendingCount());
        }
    }

    private static String reply(JSONObject request, String value) throws Exception {
        return new JSONObject().put("protocolVersion", 1).put("requestId", request.getString("requestId")).put("ok", true)
            .put("result", new JSONObject().put("value", value)).toString();
    }
}
