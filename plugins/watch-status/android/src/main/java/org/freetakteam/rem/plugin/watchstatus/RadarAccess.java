package org.freetakteam.rem.plugin.watchstatus;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.SecureRandom;
import java.util.UUID;
import java.util.function.LongSupplier;
import org.json.JSONException;
import org.json.JSONObject;

/** Process-local, read-only radar session. Never persisted or included in watch payloads. */
final class RadarAccess {
    static final long LIFETIME_MS = 8 * 60 * 60 * 1_000L;
    static final class Ticket {
        final String sessionId;
        private final long generation;
        Ticket(String sessionId, long generation) {
            this.sessionId = sessionId;
            this.generation = generation;
        }
    }

    private final LongSupplier monotonic;
    private final SecureRandom random = new SecureRandom();
    private String token = "";
    private String sessionId = "";
    private long expiresAt;
    private long generation;

    RadarAccess(LongSupplier monotonic) { this.monotonic = monotonic; }

    synchronized void renew() {
        final byte[] bytes = new byte[32];
        random.nextBytes(bytes);
        final StringBuilder value = new StringBuilder(64);
        for (byte b : bytes) value.append(String.format(java.util.Locale.ROOT, "%02x", b & 0xff));
        token = value.toString();
        sessionId = UUID.randomUUID().toString();
        generation++;
        expiresAt = monotonic.getAsLong() + LIFETIME_MS;
    }

    synchronized void revoke() {
        generation++;
        token = "";
        sessionId = "";
        expiresAt = 0;
    }

    synchronized boolean active() {
        if (!token.isEmpty() && monotonic.getAsLong() >= expiresAt) revoke();
        return !token.isEmpty();
    }

    synchronized Ticket authorize(String authorization) throws RadarFailure {
        if (!active()) throw new RadarFailure("SessionExpired", 403);
        final byte[] expected = ("Bearer " + token).getBytes(StandardCharsets.US_ASCII);
        final byte[] supplied = String.valueOf(authorization).getBytes(StandardCharsets.US_ASCII);
        if (!MessageDigest.isEqual(expected, supplied)) throw new RadarFailure("Unauthorized", 401);
        return new Ticket(sessionId, generation);
    }

    synchronized void requireCurrent(Ticket ticket) throws RadarFailure {
        if (!active() || ticket.generation != generation) throw new RadarFailure("SessionExpired", 403);
    }

    // Called only by the SDK's trusted configuration channel, never by HTTP.
    synchronized JSONObject configuration() throws JSONException {
        final boolean enabled = active();
        return new JSONObject().put("radarToken", enabled ? token : "")
            .put("radarSessionActive", enabled)
            .put("radarExpiresInMs", enabled ? Math.max(0, expiresAt - monotonic.getAsLong()) : 0);
    }
}
