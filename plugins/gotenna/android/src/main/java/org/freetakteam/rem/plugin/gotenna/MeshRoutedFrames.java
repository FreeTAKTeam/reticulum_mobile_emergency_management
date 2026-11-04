package org.freetakteam.rem.plugin.gotenna;

import java.util.HashMap;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import org.json.JSONObject;

/** REM1 keeps addressed chat separate from broadcast MECP event codes. */
final class MeshRoutedFrames {
    private static final long TTL_MS = 120_000;
    private final Map<String, Assembly> pending = new HashMap<>();

    private static final class Assembly {
        final String[] parts;
        final long startedAtMs;
        Assembly(int count, long nowMs) { parts = new String[count]; startedAtMs = nowMs; }
    }

    static String hash(String value) {
        if (!value.matches("[0-9a-f]{32}")) throw new IllegalArgumentException("REM destination and source require 32 lowercase hex digits");
        return value;
    }

    static List<String> encode(String type, String id, String source, String destination, String text) {
        hash(id);
        hash(source);
        if ("C".equals(type)) hash(destination);
        else if (!"E".equals(type) || !"-".equals(destination)) throw new IllegalArgumentException("Invalid REM1 frame type");
        final String prefix = "REM1 " + type + " " + id + " " + source + " " + destination + " ";
        final List<String> parts = MeshRelay.split(text, 180 - MeshMessage.utf8(prefix).length - 6);
        if (parts.isEmpty() || parts.size() > 24) throw new IllegalArgumentException("Message exceeds the 24-part addressed Mesh limit");
        for (int i = 0; i < parts.size(); i++) parts.set(i, prefix + (i + 1) + "/" + parts.size() + " " + parts.get(i));
        return parts;
    }

    synchronized JSONObject receive(JSONObject message, long nowMs) throws Exception {
        final Iterator<Assembly> expired = pending.values().iterator();
        while (expired.hasNext()) if (nowMs - expired.next().startedAtMs > TTL_MS) expired.remove();
        final String radioGid = message.getString("senderGid");
        if (!radioGid.matches("[0-9a-f]{12}")) throw new IllegalArgumentException("Invalid Mesh sender GID");
        final String[] fields = message.getString("text").split(" ", 7);
        if (fields.length != 7 || !"REM1".equals(fields[0])) throw new IllegalArgumentException("Unaddressed Mesh text is not a REM chat");
        final String type = fields[1];
        final String id = hash(fields[2]);
        final String source = hash(fields[3]);
        final String destination = fields[4];
        if ("C".equals(type)) hash(destination);
        else if (!"E".equals(type) || !"-".equals(destination)) throw new IllegalArgumentException("Invalid REM1 recipient/type");
        if (!fields[5].matches("[0-9]{1,2}/[0-9]{1,2}")) throw new IllegalArgumentException("Invalid REM1 fragment");
        final String[] numbering = fields[5].split("/");
        final int part = Integer.parseInt(numbering[0]);
        final int count = Integer.parseInt(numbering[1]);
        if (count < 1 || count > 24 || part < 1 || part > count || fields[6].isEmpty()) throw new IllegalArgumentException("Invalid REM1 fragment bounds");
        final String key = radioGid + ":" + type + ":" + id + ":" + source + ":" + destination;
        Assembly assembly = pending.get(key);
        if (assembly == null) {
            if (pending.size() >= 16) throw new IllegalArgumentException("Too many incomplete Mesh messages");
            assembly = new Assembly(count, nowMs);
            pending.put(key, assembly);
        }
        if (assembly.parts.length != count || (assembly.parts[part - 1] != null && !assembly.parts[part - 1].equals(fields[6]))) {
            throw new IllegalArgumentException("Conflicting REM1 fragment");
        }
        assembly.parts[part - 1] = fields[6];
        final StringBuilder body = new StringBuilder();
        for (String fragment : assembly.parts) {
            if (fragment == null) return null;
            body.append(fragment);
        }
        pending.remove(key);
        return new JSONObject().put("kind", "C".equals(type) ? "chat" : "event").put("id", id)
            .put("sourceHex", source).put("destinationHex", destination).put("text", body.toString()).put("radioGid", radioGid);
    }

    synchronized String snapshot() throws Exception {
        final JSONObject state = new JSONObject();
        for (Map.Entry<String, Assembly> entry : pending.entrySet()) {
            final org.json.JSONArray parts = new org.json.JSONArray();
            for (String part : entry.getValue().parts) parts.put(part == null ? JSONObject.NULL : part);
            state.put(entry.getKey(), new JSONObject().put("parts", parts).put("startedAtMs", entry.getValue().startedAtMs));
        }
        return state.toString();
    }

    synchronized void restore(String raw, long nowMs) throws Exception {
        final JSONObject state = new JSONObject(raw);
        if (state.length() > 16 || raw.length() > 100_000) throw new IllegalArgumentException("Invalid stored assembly bounds");
        pending.clear();
        final Iterator<String> keys = state.keys();
        while (keys.hasNext()) {
            final String key = keys.next();
            final JSONObject saved = state.getJSONObject(key);
            final long started = saved.getLong("startedAtMs");
            if (started > nowMs || nowMs - started > TTL_MS) continue;
            final org.json.JSONArray parts = saved.getJSONArray("parts");
            if (parts.length() < 1 || parts.length() > 24) throw new IllegalArgumentException("Invalid stored fragment count");
            final Assembly assembly = new Assembly(parts.length(), started);
            for (int i = 0; i < parts.length(); i++) {
                if (!parts.isNull(i)) {
                    assembly.parts[i] = parts.getString(i);
                    if (MeshMessage.utf8(assembly.parts[i]).length > 180) throw new IllegalArgumentException("Invalid stored fragment size");
                }
            }
            pending.put(key, assembly);
        }
    }

}
