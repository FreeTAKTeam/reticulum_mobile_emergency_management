package org.freetakteam.rem.plugin.bleheartrate;

final class SampleRateLimiter {
    private long lastAcceptedAtMs;

    boolean accept(long nowMs) {
        if (lastAcceptedAtMs != 0 && nowMs - lastAcceptedAtMs < 1_000L) return false;
        lastAcceptedAtMs = nowMs;
        return true;
    }
}
