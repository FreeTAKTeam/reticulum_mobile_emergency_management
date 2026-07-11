package org.freetakteam.rem.plugin.bleheartrate;

final class ReconnectPolicy {
    private static final int MAX_ATTEMPTS = 5;
    private int attempts;

    long nextDelayMs() {
        if (attempts >= MAX_ATTEMPTS) return -1L;
        attempts += 1;
        return Math.min(60_000L, 1_000L << attempts);
    }

    void reset() {
        attempts = 0;
    }
}
