package org.freetakteam.rem.plugin.gotenna;

import org.json.JSONObject;

final class MeshPositionOrder {
    static boolean isNewer(JSONObject message, JSONObject previous) throws Exception {
        if (previous == null) return true;
        final long timestamp = message.getLong("timestamp");
        final long oldTimestamp = previous.getLong("timestamp");
        if (timestamp != oldTimestamp) return timestamp > oldTimestamp;
        final int delta = (message.getInt("sequence") - previous.getInt("sequence")) & 0xffff;
        return delta > 0 && delta < 0x8000;
    }
}
