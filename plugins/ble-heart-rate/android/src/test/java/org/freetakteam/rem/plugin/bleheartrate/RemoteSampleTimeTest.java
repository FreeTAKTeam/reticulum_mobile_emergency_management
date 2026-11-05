package org.freetakteam.rem.plugin.bleheartrate;

import static org.junit.Assert.*;
import org.junit.Test;

public final class RemoteSampleTimeTest {
    @Test
    public void clampsUntrustedTimesAndPreservesValidTimes() {
        final long now = 1_000_000L;
        assertEquals(now - 300_000L, RemoteSampleTime.clamp(Long.MIN_VALUE, now));
        assertEquals(now + 300_000L, RemoteSampleTime.clamp(Long.MAX_VALUE, now));
        assertEquals(now - 100L, RemoteSampleTime.clamp(now - 100L, now));
        assertEquals(now + 100L, RemoteSampleTime.clamp(now + 100L, now));
    }
}
