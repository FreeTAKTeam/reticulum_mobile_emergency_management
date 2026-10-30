package org.freetakteam.rem.plugin.watchstatus;

import java.nio.charset.StandardCharsets;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.function.LongSupplier;
import org.json.JSONObject;

/** One authenticated full source read at a time. No location cache can outlive a grant. */
final class RadarProvider {
    private final HostRequestBroker host;
    private final RadarAccess access;
    private final LongSupplier wallClock;
    private final LongSupplier monotonic;
    private final AtomicBoolean inFlight = new AtomicBoolean();
    private long lastStarted = Long.MIN_VALUE;
    private long sequence;

    RadarProvider(HostRequestBroker host, RadarAccess access, LongSupplier wallClock, LongSupplier monotonic) {
        this.host = host;
        this.access = access;
        this.wallClock = wallClock;
        this.monotonic = monotonic;
    }

    String get(String authorization, String requestId) throws RadarFailure {
        if (requestId == null || !requestId.matches("[A-Za-z0-9_-]{1,64}")) throw new RadarFailure("InvalidRequest", 400);
        final RadarAccess.Ticket ticket = access.authorize(authorization);
        if (!inFlight.compareAndSet(false, true)) throw new RadarFailure("Busy", 429);
        try {
            final long now = monotonic.getAsLong();
            if (lastStarted != Long.MIN_VALUE && now - lastStarted < 500) throw new RadarFailure("RateLimited", 429);
            lastStarted = now;
            final JSONObject before = host.request("map.observer.snapshot", new JSONObject()).get();
            access.requireCurrent(ticket);
            final JSONObject nearby = host.request("map.markers.nearby", new JSONObject()
                .put("radiusM", RadarSnapshot.RANGE_M).put("limit", RadarSnapshot.MAX_MARKERS)).get();
            access.requireCurrent(ticket);
            final JSONObject after = host.request("map.observer.snapshot", new JSONObject()).get();
            access.requireCurrent(ticket);
            if (++sequence >= 9_007_199_254_740_991L) throw new RadarFailure("SessionExpired", 403);
            final String body = RadarSnapshot.build(before, nearby, after, requestId, ticket.sessionId,
                sequence, wallClock.getAsLong()).toString();
            if (body.getBytes(StandardCharsets.UTF_8).length > 65_536) throw new RadarFailure("ResponseTooLarge", 503);
            access.requireCurrent(ticket);
            return body;
        } catch (ExecutionException error) {
            final Throwable cause = error.getCause();
            if (cause instanceof RadarFailure) {
                final RadarFailure failure = (RadarFailure) cause;
                if ("PermissionDenied".equals(failure.code) && !access.revokeCurrent(ticket)) access.requireCurrent(ticket);
                throw failure;
            }
            throw new RadarFailure("HostError", 503);
        } catch (InterruptedException error) {
            Thread.currentThread().interrupt();
            throw new RadarFailure("HostUnavailable", 503);
        } catch (RadarFailure failure) {
            throw failure;
        } catch (Exception error) {
            throw new RadarFailure("InvalidSource", 503);
        } finally {
            inFlight.set(false);
        }
    }
}
