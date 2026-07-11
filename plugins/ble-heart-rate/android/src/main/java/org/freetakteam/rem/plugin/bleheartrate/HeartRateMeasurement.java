package org.freetakteam.rem.plugin.bleheartrate;

public record HeartRateMeasurement(int bpm, boolean contactSupported, boolean contactDetected) {
    public static HeartRateMeasurement parse(byte[] data) {
        if (data == null || data.length < 2) return null;
        final int flags = data[0] & 0xff;
        final boolean sixteenBit = (flags & 0x01) != 0;
        if (sixteenBit && data.length < 3) return null;
        final int bpm = sixteenBit
            ? ((data[2] & 0xff) << 8) | (data[1] & 0xff)
            : data[1] & 0xff;
        if (bpm < 1 || bpm > 240) return null;
        final boolean contactSupported = (flags & 0x04) != 0;
        final boolean contactDetected = contactSupported && (flags & 0x02) != 0;
        return new HeartRateMeasurement(bpm, contactSupported, contactDetected);
    }
}
