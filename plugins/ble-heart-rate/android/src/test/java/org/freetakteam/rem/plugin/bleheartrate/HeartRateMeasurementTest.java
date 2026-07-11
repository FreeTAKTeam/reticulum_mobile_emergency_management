package org.freetakteam.rem.plugin.bleheartrate;

import static org.junit.Assert.*;
import org.junit.Test;

public final class HeartRateMeasurementTest {
    @Test
    public void parsesEightBitBpmAndContactFlags() {
        final HeartRateMeasurement value = HeartRateMeasurement.parse(new byte[] {0x06, 72});
        assertNotNull(value);
        assertEquals(72, value.bpm());
        assertTrue(value.contactSupported());
        assertTrue(value.contactDetected());
    }

    @Test
    public void parsesSixteenBitBpm() {
        final HeartRateMeasurement value = HeartRateMeasurement.parse(new byte[] {0x01, (byte) 180, 0});
        assertNotNull(value);
        assertEquals(180, value.bpm());
    }

    @Test
    public void rejectsMalformedAndImplausibleMeasurements() {
        assertNull(HeartRateMeasurement.parse(null));
        assertNull(HeartRateMeasurement.parse(new byte[] {0x01, 40}));
        assertNull(HeartRateMeasurement.parse(new byte[] {0x00, 0}));
        assertNull(HeartRateMeasurement.parse(new byte[] {0x01, (byte) 250, 0}));
    }
}
