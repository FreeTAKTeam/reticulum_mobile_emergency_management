package org.freetakteam.rem.plugin.bleheartrate;

final class SharingPolicy {
    private SharingPolicy() {}

    static boolean shouldSend(String destination, long lastSentAtMs, long nowMs, long intervalMs) {
        return intervalMs > 0L
            && lastSentAtMs >= -1L
            && nowMs >= 0L
            && nowMs >= lastSentAtMs
            && destination != null
            && destination.matches("[0-9a-f]{32}")
            && (lastSentAtMs == -1L || nowMs - lastSentAtMs >= intervalMs);
    }
}
