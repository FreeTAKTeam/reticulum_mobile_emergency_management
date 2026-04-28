package network.reticulum.emergency.wearables;

import static org.junit.Assert.assertEquals;

import org.junit.Test;

public class BleHeartRateClientTest {
    @Test
    public void parseHeartRateReadsUint8Payload() {
        assertEquals(82, BleHeartRateClient.parseHeartRate(new byte[] { 0x00, 0x52 }));
    }

    @Test
    public void parseHeartRateReadsUint16LittleEndianPayload() {
        assertEquals(82, BleHeartRateClient.parseHeartRate(new byte[] { 0x01, 0x52, 0x00 }));
    }

    @Test
    public void parseHeartRateRejectsMissingPayloads() {
        assertEquals(-1, BleHeartRateClient.parseHeartRate(null));
        assertEquals(-1, BleHeartRateClient.parseHeartRate(new byte[] {}));
        assertEquals(-1, BleHeartRateClient.parseHeartRate(new byte[] { 0x00 }));
        assertEquals(-1, BleHeartRateClient.parseHeartRate(new byte[] { 0x01, 0x52 }));
    }

    @Test
    public void parseHeartRateRejectsOutOfRangeValues() {
        assertEquals(-1, BleHeartRateClient.parseHeartRate(new byte[] { 0x00, 0x00 }));
        assertEquals(-1, BleHeartRateClient.parseHeartRate(new byte[] { 0x00, (byte) 0xF5 }));
        assertEquals(-1, BleHeartRateClient.parseHeartRate(new byte[] { 0x01, (byte) 0xF1, 0x00 }));
    }

    @Test
    public void parseHeartRateAcceptsRealisticValues() {
        assertEquals(60, BleHeartRateClient.parseHeartRate(new byte[] { 0x00, 0x3C }));
        assertEquals(82, BleHeartRateClient.parseHeartRate(new byte[] { 0x00, 0x52 }));
        assertEquals(120, BleHeartRateClient.parseHeartRate(new byte[] { 0x00, 0x78 }));
        assertEquals(180, BleHeartRateClient.parseHeartRate(new byte[] { 0x00, (byte) 0xB4 }));
    }
}
