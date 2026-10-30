package org.freetakteam.rem.plugin.watchstatus;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.SecureRandom;
import java.util.UUID;
import org.json.JSONException;
import org.json.JSONObject;

/** Durable phone pairing, with separate process-local read tickets. Never sent to the watch. */
final class RadarAccess {
    interface Credentials {
        String read();
        void write(String token);
    }
    static final class Ticket {
        final String sessionId;
        private final long generation;
        private final long pairingGeneration;
        Ticket(String sessionId, long generation, long pairingGeneration) {
            this.sessionId = sessionId;
            this.generation = generation;
            this.pairingGeneration = pairingGeneration;
        }
    }

    private final Credentials credentials;
    private final SecureRandom random = new SecureRandom();
    private String token = "";
    private String sessionId = "";
    private boolean enabled;
    private long generation;
    private long pairingGeneration;

    RadarAccess(Credentials credentials) { this.credentials = credentials; }

    synchronized void resume() {
        if (active()) return;
        final String saved = credentials.read();
        if (saved != null && saved.matches("[a-f0-9]{64}")) {
            token = saved;
            startSession();
        } else {
            renew();
        }
    }

    synchronized void pause() {
        generation++;
        enabled = false;
        sessionId = "";
    }

    private void startSession() {
        sessionId = UUID.randomUUID().toString();
        generation++;
        enabled = true;
    }

    synchronized void renew() {
        final byte[] bytes = new byte[32];
        random.nextBytes(bytes);
        final StringBuilder value = new StringBuilder(64);
        for (byte b : bytes) value.append(String.format(java.util.Locale.ROOT, "%02x", b & 0xff));
        // Persist before exposing a new pairing. A failed write must not leave access active.
        pause();
        credentials.write(value.toString());
        token = value.toString();
        pairingGeneration++;
        startSession();
    }

    synchronized void revoke() {
        pause();
        token = "";
        pairingGeneration++;
        credentials.write("");
    }

    synchronized boolean revokeCurrent(Ticket ticket) {
        if (ticket.generation != generation) return false;
        revoke();
        return true;
    }

    synchronized boolean active() {
        return enabled && !token.isEmpty();
    }

    synchronized Ticket authorize(String authorization) throws RadarFailure {
        if (!active()) throw unavailable();
        final byte[] expected = ("Bearer " + token).getBytes(StandardCharsets.US_ASCII);
        final byte[] supplied = String.valueOf(authorization).getBytes(StandardCharsets.US_ASCII);
        if (!MessageDigest.isEqual(expected, supplied)) throw new RadarFailure("Unauthorized", 401);
        return new Ticket(sessionId, generation, pairingGeneration);
    }

    synchronized void requireCurrent(Ticket ticket) throws RadarFailure {
        if (ticket.pairingGeneration != pairingGeneration) throw new RadarFailure("SessionExpired", 403);
        if (!active() || ticket.generation != generation) throw unavailable();
    }

    private RadarFailure unavailable() {
        return token.isEmpty() ? new RadarFailure("SessionExpired", 403) : new RadarFailure("HostUnavailable", 503);
    }

    // Called only by the SDK's trusted configuration channel, never by HTTP.
    synchronized JSONObject configuration() throws JSONException {
        final String saved = credentials.read();
        final boolean paired = saved != null && saved.matches("[a-f0-9]{64}");
        return new JSONObject().put("radarToken", paired ? saved : "")
            .put("radarSessionActive", active())
            .put("radarPairingPersistent", paired);
    }
}
