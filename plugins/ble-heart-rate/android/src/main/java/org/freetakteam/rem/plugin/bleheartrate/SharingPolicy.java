package org.freetakteam.rem.plugin.bleheartrate;

final class SharingPolicy {
    private SharingPolicy() {}

    static boolean shouldSend(String destination, long lastSentAtMs, long nowMs, long intervalMs) {
        return destination != null
            && destination.matches("[0-9a-f]{32}")
            && (lastSentAtMs == 0L || nowMs - lastSentAtMs >= intervalMs);
    }
}
