package org.freetakteam.rem.plugin.bleheartrate;

import static org.junit.Assert.*;
import org.junit.Test;

public final class SampleRateLimiterTest {
    @Test
    public void acceptsAtMostOneSamplePerSecond() {
        final SampleRateLimiter limiter = new SampleRateLimiter();
        assertTrue(limiter.accept(0L));
        assertFalse(limiter.accept(999L));
        assertTrue(limiter.accept(1_000L));
    }
}
