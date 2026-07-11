package org.freetakteam.rem.plugin.bleheartrate;

import static org.junit.Assert.*;
import org.junit.Test;

public final class SharingPolicyTest {
    private static final String DESTINATION = "0123456789abcdef0123456789abcdef";

    @Test
    public void rejectsUnconfiguredSharingAndRateLimitsConfiguredSharing() {
        assertFalse(SharingPolicy.shouldSend("", 0L, 30_000L, 30_000L));
        assertFalse(SharingPolicy.shouldSend("not-a-destination", 0L, 30_000L, 30_000L));
        assertTrue(SharingPolicy.shouldSend(DESTINATION, 0L, 30_000L, 30_000L));
        assertFalse(SharingPolicy.shouldSend(DESTINATION, 30_000L, 59_999L, 30_000L));
        assertTrue(SharingPolicy.shouldSend(DESTINATION, 30_000L, 60_000L, 30_000L));
    }
}
