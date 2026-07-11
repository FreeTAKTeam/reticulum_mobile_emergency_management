package org.freetakteam.rem.plugin.bleheartrate;

import static org.junit.Assert.*;
import org.junit.Test;

public final class SampleRateLimiterTest {
    @Test
    public void acceptsAtMostOneSamplePerSecond() {
        final SampleRateLimiter limiter = new SampleRateLimiter();
        assertTrue(limiter.accept(10_000L));
        assertFalse(limiter.accept(10_999L));
        assertTrue(limiter.accept(11_000L));
    }
}
