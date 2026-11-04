package org.freetakteam.rem.plugin.gotenna;

import java.io.ByteArrayOutputStream;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

/** goTenna Mesh DLE framing; see sybip/pyGT and gotenna-test protocol vectors. */
final class MeshFraming {
    static final int MAX_BYTES = 4096;
    private final ByteArrayOutputStream body = new ByteArrayOutputStream();
    private boolean open;
    private boolean escape;

    static int crc(byte[] data) {
        int crc = 0;
        for (byte value : data) {
            crc ^= (value & 255) << 8;
            for (int bit = 0; bit < 8; bit++) {
                crc = ((crc << 1) ^ ((crc & 0x8000) == 0 ? 0 : 0x1021)) & 0xffff;
            }
        }
        return crc;
    }

    static byte[] withCrc(byte[] data) {
        final ByteArrayOutputStream out = new ByteArrayOutputStream();
        out.write(data, 0, data.length);
        final int checksum = crc(data);
        out.write(checksum >> 8);
        out.write(checksum);
        return out.toByteArray();
    }

    static byte[] checked(byte[] data) {
        if (data.length < 2 || crc(Arrays.copyOf(data, data.length - 2))
            != (((data[data.length - 2] & 255) << 8) | (data[data.length - 1] & 255))) {
            throw new IllegalArgumentException("Mesh checksum mismatch");
        }
        return Arrays.copyOf(data, data.length - 2);
    }

    static byte[] encode(int opcode, int sequence, byte[] payload) {
        if (payload.length > 512) throw new IllegalArgumentException("Mesh command is too large");
        final ByteArrayOutputStream out = new ByteArrayOutputStream();
        out.write(opcode);
        out.write(sequence);
        out.write(payload, 0, payload.length);
        final byte[] data = withCrc(out.toByteArray());
        out.reset();
        out.write(0x10);
        out.write(0x02);
        for (byte value : data) {
            out.write(value);
            if (value == 0x10) out.write(value);
        }
        out.write(0x10);
        out.write(0x03);
        return out.toByteArray();
    }

    synchronized List<byte[]> receive(byte[] chunk) {
        final List<byte[]> frames = new ArrayList<>();
        for (byte value : chunk) {
            final int next = value & 255;
            if (escape) {
                escape = false;
                if (next == 2) {
                    body.reset();
                    open = true;
                } else if (next == 3) {
                    if (open) {
                        open = false;
                        final byte[] decoded = checked(body.toByteArray());
                        if (decoded.length < 2) throw new IllegalArgumentException("Mesh frame lacks header");
                        frames.add(decoded);
                    }
                    body.reset();
                } else if (next == 0x10 && open) {
                    body.write(next);
                } else {
                    open = false;
                    body.reset();
                    throw new IllegalArgumentException("Invalid Mesh escape");
                }
            } else if (next == 0x10) {
                escape = true;
            } else if (open) {
                body.write(next);
            }
            if (body.size() > MAX_BYTES) {
                open = false;
                body.reset();
                throw new IllegalArgumentException("Mesh receive buffer exceeded");
            }
        }
        return frames;
    }
}
