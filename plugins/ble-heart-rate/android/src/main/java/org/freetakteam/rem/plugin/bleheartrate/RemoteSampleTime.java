package org.freetakteam.rem.plugin.bleheartrate;

final class RemoteSampleTime {
    private static final long MAX_SKEW_MS = 5 * 60_000L;

    private RemoteSampleTime() {}

    static long clamp(long measuredAtMs, long nowMs) {
        return Math.max(nowMs - MAX_SKEW_MS, Math.min(nowMs + MAX_SKEW_MS, measuredAtMs));
    }
}
