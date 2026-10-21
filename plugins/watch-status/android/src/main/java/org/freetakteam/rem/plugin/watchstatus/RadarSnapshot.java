package org.freetakteam.rem.plugin.watchstatus;

import java.time.Instant;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONObject;

/** Projection of the current native map wire contract, without notes or coordinates. */
final class RadarSnapshot {
    static final int MAX_MARKERS = 50;
    static final double RANGE_M = 500;
    private static final double EARTH_MEAN_RADIUS_M = 6_371_008.8;
    // Generic REM pins keep their map fill. Only canonical NATO catalog IDs
    // carry affiliation; do not infer it from names, categories or other icons.
    static final int REM_PIN_COLOR = 0x64beff;

    static int markerColor(String symbol) {
        switch (symbol) {
            case "friendly": return 0x007aff;
            case "hostile": return 0xff303b;
            case "neutral": return 0x00d43b;
            case "unknown": return 0xffe600;
            default: return REM_PIN_COLOR;
        }
    }

    static JSONObject build(JSONObject before, JSONObject nearby, JSONObject after,
            String requestId, String sessionId, long sequence, long now) throws Exception {
        requireSameObserver(before, after);
        if (!(nearby.opt("radiusM") instanceof Number) || nearby.getDouble("radiusM") != RANGE_M ||
                !(nearby.opt("available") instanceof Boolean) ||
                nearby.getBoolean("available") != before.getBoolean("available")) throw invalid();
        final JSONArray items = nearby.getJSONArray("items");
        if (items.length() > MAX_MARKERS) throw invalid();
        final boolean available = before.getBoolean("available");
        if (!available && items.length() != 0) throw invalid();
        final List<JSONObject> markers = new ArrayList<>();
        final Set<String> ids = new HashSet<>();
        if (available) {
            final double lat = coordinate(before, "lat", 90);
            final double lon = coordinate(before, "lon", 180);
            for (int i = 0; i < items.length(); i++) {
                final JSONObject item = items.getJSONObject(i);
                final String id = item.getString("object_destination_hash");
                if (id.isEmpty() || id.length() > 128 || !ids.add(id)) throw invalid();
                final JSONObject position = item.getJSONObject("position");
                final double targetLat = coordinate(position, "lat", 90);
                final double targetLon = coordinate(position, "lon", 180);
                final double distance = distance(lat, lon, targetLat, targetLon);
                if (distance > RANGE_M) continue;
                markers.add(new JSONObject().put("id", id).put("label", label(item.getString("name")))
                    .put("color", markerColor(item.optString("symbol", ""))).put("distanceM", distance)
                    .put("bearingDegTrue", distance < 0.5 ? JSONObject.NULL : bearing(lat, lon, targetLat, targetLon))
                    .put("markerUpdatedAtUnixMs", timestamp(item.opt("updated_at"))));
            }
        }
        markers.sort(Comparator.comparingDouble((JSONObject item) -> item.optDouble("distanceM"))
            .thenComparing(item -> item.optString("id")));
        final JSONArray projected = new JSONArray();
        for (JSONObject marker : markers) projected.put(marker);
        final Object positionTime = available ? knownTime(before.opt("updatedAtMs")) : JSONObject.NULL;
        return new JSONObject().put("protocolVersion", 1).put("requestId", requestId)
            .put("snapshotId", sessionId + ":" + sequence).put("sessionId", sessionId).put("sequence", sequence)
            .put("generatedAtUnixMs", now)
            .put("observer", new JSONObject().put("positionValid", available)
                .put("positionUpdatedAtUnixMs", positionTime)
                .put("accuracyM", available ? accuracy(before) : JSONObject.NULL).put("bearingReference", "true"))
            .put("markers", projected).put("truncated", items.length() == MAX_MARKERS);
    }

    static void requireSameObserver(JSONObject a, JSONObject b) throws Exception {
        if (!(a.opt("available") instanceof Boolean) || !(b.opt("available") instanceof Boolean) ||
                a.getBoolean("available") != b.getBoolean("available")) throw changed();
        final JSONObject first = a.getJSONObject("observer");
        final JSONObject second = b.getJSONObject("observer");
        if (!"localDevice".equals(first.optString("kind")) || !"localDevice".equals(second.optString("kind")) ||
                !Objects.equals(first.opt("identityHex"), second.opt("identityHex")) ||
                !Objects.equals(first.opt("lxmfDestinationHex"), second.opt("lxmfDestinationHex"))) throw changed();
        if (a.getBoolean("available")) {
            if (!"WGS84".equals(a.optString("datum")) || !"WGS84".equals(b.optString("datum")) ||
                    !"device".equals(a.optString("source")) || !"device".equals(b.optString("source")) ||
                    coordinate(a, "lat", 90) != coordinate(b, "lat", 90) ||
                    coordinate(a, "lon", 180) != coordinate(b, "lon", 180) ||
                    !Objects.equals(a.opt("updatedAtMs"), b.opt("updatedAtMs")) ||
                    !Objects.equals(accuracy(a), accuracy(b))) throw changed();
        }
    }

    private static Object accuracy(JSONObject observer) throws RadarFailure {
        final Object value = observer.opt("horizontalAccuracyM");
        if (value == null || value == JSONObject.NULL) return JSONObject.NULL;
        if (!(value instanceof Number)) throw invalid();
        final double metres = ((Number) value).doubleValue();
        if (!Double.isFinite(metres) || metres < 0) throw invalid();
        return metres;
    }

    private static double coordinate(JSONObject object, String key, double limit) throws Exception {
        final Object value = object.opt(key);
        if (!(value instanceof Number)) throw invalid();
        final double number = ((Number) value).doubleValue();
        if (!Double.isFinite(number) || Math.abs(number) > limit) throw invalid();
        return number;
    }

    static double distance(double lat, double lon, double targetLat, double targetLon) {
        final double p1 = Math.toRadians(lat);
        final double p2 = Math.toRadians(targetLat);
        final double dp = p2 - p1;
        final double dl = Math.toRadians(targetLon - lon);
        final double h = Math.pow(Math.sin(dp / 2), 2) + Math.cos(p1) * Math.cos(p2) * Math.pow(Math.sin(dl / 2), 2);
        return EARTH_MEAN_RADIUS_M * 2 * Math.atan2(Math.sqrt(Math.max(0, h)), Math.sqrt(Math.max(0, 1 - h)));
    }

    static double bearing(double lat, double lon, double targetLat, double targetLon) {
        final double p1 = Math.toRadians(lat);
        final double p2 = Math.toRadians(targetLat);
        final double dl = Math.toRadians(targetLon - lon);
        final double angle = Math.toDegrees(Math.atan2(Math.sin(dl) * Math.cos(p2),
            Math.cos(p1) * Math.sin(p2) - Math.sin(p1) * Math.cos(p2) * Math.cos(dl)));
        return (angle + 360) % 360;
    }

    private static Object knownTime(Object value) {
        if (!(value instanceof Number)) return JSONObject.NULL;
        final double time = ((Number) value).doubleValue();
        return Double.isFinite(time) && time > 0 && time <= 9_007_199_254_740_991L && time == Math.floor(time)
            ? ((Number) value).longValue() : JSONObject.NULL;
    }

    private static Object timestamp(Object value) {
        if (!(value instanceof String)) return JSONObject.NULL;
        try { return knownTime(Instant.parse((String) value).toEpochMilli()); }
        catch (RuntimeException unknown) { return JSONObject.NULL; }
    }

    private static String label(String value) throws RadarFailure {
        if (value.codePointCount(0, value.length()) > 128) throw invalid();
        final StringBuilder out = new StringBuilder();
        value.codePoints().filter(c -> !Character.isISOControl(c) && Character.getType(c) != Character.FORMAT)
            .limit(32).forEach(out::appendCodePoint);
        return out.toString();
    }

    private static RadarFailure invalid() { return new RadarFailure("InvalidSource", 503); }
    private static RadarFailure changed() { return new RadarFailure("ObserverChanged", 503); }
}
