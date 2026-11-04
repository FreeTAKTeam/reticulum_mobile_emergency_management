package org.freetakteam.rem.plugin.gotenna;

import java.io.ByteArrayOutputStream;
import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.Map;
import org.json.JSONObject;

final class MeshMessage {
    static byte[] tlvs(int kind, byte[] value) {
        if (value.length > 255) throw new IllegalArgumentException("Mesh field exceeds 255 bytes");
        final byte[] result = new byte[value.length + 2];
        result[0] = (byte) kind;
        result[1] = (byte) value.length;
        System.arraycopy(value, 0, result, 2, value.length);
        return result;
    }

    static byte[] join(byte[]... parts) {
        final ByteArrayOutputStream out = new ByteArrayOutputStream();
        for (byte[] part : parts) out.write(part, 0, part.length);
        return out.toByteArray();
    }

    static Map<Integer, byte[]> fields(byte[] bytes) {
        if (bytes.length > MeshFraming.MAX_BYTES) throw new IllegalArgumentException("Mesh payload too large");
        final Map<Integer, byte[]> values = new LinkedHashMap<>();
        for (int offset = 0; offset < bytes.length;) {
            if (offset + 2 > bytes.length) throw new IllegalArgumentException("Truncated Mesh field");
            final int kind = bytes[offset++] & 255;
            final int size = bytes[offset++] & 255;
            if (offset + size > bytes.length || values.containsKey(kind)) {
                throw new IllegalArgumentException("Invalid or duplicate Mesh field");
            }
            values.put(kind, Arrays.copyOfRange(bytes, offset, offset + size));
            offset += size;
        }
        return values;
    }

    static byte[] required(Map<Integer, byte[]> fields, int key) {
        final byte[] value = fields.get(key);
        if (value == null) throw new IllegalArgumentException("Missing Mesh field " + key);
        return value;
    }

    static String text(byte[] data) throws CharacterCodingException {
        return StandardCharsets.UTF_8.newDecoder().decode(ByteBuffer.wrap(data)).toString();
    }

    static byte[] utf8(String text) {
        return text.getBytes(StandardCharsets.UTF_8);
    }

    static long gid(String hex) {
        if (!hex.matches("[0-9a-fA-F]{12}")) throw new IllegalArgumentException("GID must contain 12 hex digits");
        final long value = Long.parseLong(hex, 16);
        if (value == 0) throw new IllegalArgumentException("GID must be nonzero");
        return value;
    }

    static void coordinates(double lat, double lon) {
        if (!Double.isFinite(lat) || !Double.isFinite(lon) || Math.abs(lat) > 90 || Math.abs(lon) > 180) {
            throw new IllegalArgumentException("Invalid Mesh coordinates");
        }
    }

    static byte[] shout(long gid, long timestamp, int sequence, String nickname, String text,
        Double lat, Double lon, String label) {
        if (gid <= 0 || gid >= (1L << 48) || timestamp < 0 || timestamp > 0xffffffffL) {
            throw new IllegalArgumentException("Invalid Mesh identity or timestamp");
        }
        if (utf8(text).length < 1 || utf8(text).length > 180 || utf8(nickname).length > 32) {
            throw new IllegalArgumentException("Text requires 1–180 UTF-8 bytes; nickname allows 32");
        }
        byte[] content = join(tlvs(1, utf8(lat == null ? "0" : "1")), tlvs(3, utf8(nickname)), tlvs(4, utf8(text)));
        if (lat != null) {
            if (lon == null) throw new IllegalArgumentException("Longitude required");
            coordinates(lat, lon);
            final byte[] location = join(tlvs(7, utf8(label)), tlvs(8, ByteBuffer.allocate(8).putDouble(lat).array()),
                tlvs(9, ByteBuffer.allocate(8).putDouble(lon).array()), tlvs(10, utf8("3")),
                tlvs(0x13, ByteBuffer.allocate(4).putInt((int) timestamp).array()));
            content = join(content, tlvs(6, location));
        }
        final byte[] header = ByteBuffer.allocate(16).put((byte) 0).putLong(gid).putInt((int) timestamp)
            .putShort((short) sequence).put((byte) 0).array();
        return join(tlvs(6, new byte[] {2, 0x3f, (byte) 0xff}),
            tlvs(5, join(tlvs(0xfb, header), MeshFraming.withCrc(content))), tlvs(0x22, new byte[] {1}));
    }

    static JSONObject decode(byte[] payload) throws Exception {
        final Map<Integer, byte[]> envelope = fields(payload);
        final byte[] destination = required(envelope, 6);
        if (destination.length < 3 || destination[1] != 0x3f || destination[2] != (byte) 0xff
            || (destination[0] & 255) > 3) throw new IllegalArgumentException("Unsupported Mesh application");
        final byte[] data = required(envelope, 5);
        if (data.length < 20 || data[0] != (byte) 0xfb || data[1] != 16) {
            throw new IllegalArgumentException("Invalid Mesh message header");
        }
        if (data[2] != 0) throw new IllegalArgumentException("Encrypted Mesh content is stored without decoding");
        final ByteBuffer header = ByteBuffer.wrap(data, 3, 15);
        final long gid = header.getLong();
        if (gid <= 0 || gid >= (1L << 48)) throw new IllegalArgumentException("Invalid sender GID");
        final long timestamp = Integer.toUnsignedLong(header.getInt());
        final int sequence = Short.toUnsignedInt(header.getShort());
        final Map<Integer, byte[]> content = fields(MeshFraming.checked(Arrays.copyOfRange(data, 18, data.length)));
        final String type = text(required(content, 1));
        if (!"0".equals(type) && !"1".equals(type)) throw new IllegalArgumentException("Unsupported Mesh content type");
        final String nickname = text(content.getOrDefault(3, new byte[0]));
        final String message = text(required(content, 4));
        if (utf8(nickname).length > 128 || utf8(message).length > 255) throw new IllegalArgumentException("Mesh text too large");
        final JSONObject result = new JSONObject().put("senderGid", String.format(java.util.Locale.ROOT, "%012x", gid))
            .put("timestamp", timestamp).put("sequence", sequence).put("nickname", nickname).put("text", message);
        if ("1".equals(type)) {
            final Map<Integer, byte[]> location = fields(required(content, 6));
            final byte[] latitude = required(location, 8);
            final byte[] longitude = required(location, 9);
            if (latitude.length != 8 || longitude.length != 8) throw new IllegalArgumentException("Invalid Mesh coordinate width");
            final double lat = ByteBuffer.wrap(latitude).getDouble();
            final double lon = ByteBuffer.wrap(longitude).getDouble();
            coordinates(lat, lon);
            result.put("lat", lat).put("lon", lon).put("label", text(location.getOrDefault(7, new byte[0])));
        }
        return result;
    }
}
