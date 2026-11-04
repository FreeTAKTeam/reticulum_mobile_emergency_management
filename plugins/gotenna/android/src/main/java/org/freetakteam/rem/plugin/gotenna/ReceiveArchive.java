package org.freetakteam.rem.plugin.gotenna;

import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import android.content.SharedPreferences;
import org.json.JSONObject;

final class ReceiveArchive {
    private final File file;
    private final SharedPreferences preferences;

    ReceiveArchive(File directory, SharedPreferences preferences) {
        file = new File(directory, "mesh-inbox.jsonl");
        this.preferences = preferences;
    }

    static String hex(byte[] bytes) {
        final StringBuilder out = new StringBuilder(bytes.length * 2);
        for (byte value : bytes) out.append(String.format(java.util.Locale.ROOT, "%02x", value & 255));
        return out.toString();
    }

    void save(byte[] raw, JSONObject decoded) throws Exception {
        final String id = hex(MessageDigest.getInstance("SHA-256").digest(raw));
        if (id.equals(preferences.getString("lastReceivedId", ""))) return;
        final JSONObject record = new JSONObject().put("id", id).put("receivedAtMs", System.currentTimeMillis())
            .put("rawHex", hex(raw)).put("message", decoded);
        final byte[] line = (record + "\n").getBytes(StandardCharsets.UTF_8);
        if (file.length() + line.length > 4 * 1024 * 1024) throw new IOException("Mesh receive archive full; radio queue was preserved");
        try (FileOutputStream output = new FileOutputStream(file, true)) {
            output.write(line);
            output.getFD().sync();
        }
        if (!preferences.edit().putString("lastReceivedId", id).commit()) {
            throw new IOException("Mesh receive checkpoint could not be saved; radio queue was preserved");
        }
    }
}
