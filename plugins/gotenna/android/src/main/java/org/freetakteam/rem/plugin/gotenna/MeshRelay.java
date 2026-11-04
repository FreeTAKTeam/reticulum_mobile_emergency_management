package org.freetakteam.rem.plugin.gotenna;

import java.util.ArrayList;
import java.util.List;
import org.json.JSONObject;

/** Outgoing public bridge envelopes preserve the gateway identity and the REM source label. */
final class MeshRelay {
    static boolean isEcho(JSONObject message) {
        return message.optString("text").startsWith("[REM:") || message.optString("label").startsWith("[REM:");
    }

    static List<byte[]> encode(JSONObject item, long gid, String nickname, int sequence, long nowMs) throws Exception {
        final String id = item.getString("id");
        final String source = item.getString("source");
        if (!id.matches("[0-9a-f]{64}") || !source.matches("[0-9a-f]{32}")) {
            throw new IllegalArgumentException("Invalid relay provenance");
        }
        final JSONObject payload = item.getJSONObject("payload");
        final String prefix = "[REM:" + id.substring(0, 32) + "] " + source + " ";
        final List<byte[]> result = new ArrayList<>();
        if ("position".equals(item.getString("kind"))) {
            final String callsign = payload.getString("callsign");
            final String label = callsign;
            if (MeshMessage.utf8(label).length > 64) throw new IllegalArgumentException("Position label too long for Mesh");
            result.add(MeshMessage.shout(gid, nowMs / 1000, sequence, nickname, prefix + "position",
                payload.getDouble("lat"), payload.getDouble("lon"), label));
        } else if ("text".equals(item.getString("kind")) || "event".equals(item.getString("kind"))) {
            final boolean chat = "text".equals(item.getString("kind"));
            final String wireId = chat ? payload.getString("messageId").substring(0, 32) : id.substring(0, 32);
            final List<String> parts = MeshRoutedFrames.encode(chat ? "C" : "E", wireId, source,
                chat ? payload.getString("destinationHex") : "-", chat ? payload.getString("text") : payload.getString("code"));
            for (int i = 0; i < parts.size(); i++) {
                result.add(MeshMessage.shout(gid, nowMs / 1000, sequence + i, nickname, parts.get(i), null, null, ""));
            }
        } else throw new IllegalArgumentException("Unsupported relay item");
        return result;
    }

    static List<String> split(String text, int maxBytes) {
        if (maxBytes < 4) throw new IllegalArgumentException("Mesh prefix too long");
        final List<String> result = new ArrayList<>();
        StringBuilder part = new StringBuilder();
        int size = 0;
        for (int offset = 0; offset < text.length();) {
            final int codePoint = text.codePointAt(offset);
            final String character = new String(Character.toChars(codePoint));
            final int bytes = MeshMessage.utf8(character).length;
            if (size + bytes > maxBytes) { result.add(part.toString()); part = new StringBuilder(); size = 0; }
            part.append(character);
            size += bytes;
            offset += Character.charCount(codePoint);
        }
        if (part.length() != 0) result.add(part.toString());
        return result;
    }
}
