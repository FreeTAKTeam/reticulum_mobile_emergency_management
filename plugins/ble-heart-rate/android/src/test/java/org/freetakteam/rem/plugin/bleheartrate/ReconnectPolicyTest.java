package org.freetakteam.rem.plugin.bleheartrate;

import static org.junit.Assert.*;
import org.junit.Test;

public final class ReconnectPolicyTest {
    @Test
    public void retriesWithBoundedBackoffAndStops() {
        final ReconnectPolicy policy = new ReconnectPolicy();
        assertEquals(2_000L, policy.nextDelayMs());
        assertEquals(4_000L, policy.nextDelayMs());
        assertEquals(8_000L, policy.nextDelayMs());
        assertEquals(16_000L, policy.nextDelayMs());
        assertEquals(32_000L, policy.nextDelayMs());
        assertEquals(-1L, policy.nextDelayMs());
        policy.reset();
        assertEquals(2_000L, policy.nextDelayMs());
    }
}
