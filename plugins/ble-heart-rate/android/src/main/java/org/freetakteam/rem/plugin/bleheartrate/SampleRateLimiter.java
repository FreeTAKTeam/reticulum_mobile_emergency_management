package org.freetakteam.rem.plugin.bleheartrate;

final class SampleRateLimiter {
    private boolean hasAcceptedSample;
    private long lastAcceptedAtMs;

    synchronized boolean accept(long nowMs) {
        if (nowMs < 0L) return false;
        if (hasAcceptedSample
            && (nowMs < lastAcceptedAtMs || nowMs - lastAcceptedAtMs < 1_000L)) {
            return false;
        }
        hasAcceptedSample = true;
        lastAcceptedAtMs = nowMs;
        return true;
    }
}
