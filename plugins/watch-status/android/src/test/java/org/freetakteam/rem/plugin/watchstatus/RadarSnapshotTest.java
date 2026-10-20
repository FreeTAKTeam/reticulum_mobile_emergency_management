package org.freetakteam.rem.plugin.watchstatus;

import static org.junit.Assert.*;
import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.Test;

public class RadarSnapshotTest {
    static final long NOW = 1_792_493_152_000L;

    static JSONObject observer() throws Exception {
        return new JSONObject().put("available", true).put("source", "device").put("datum", "WGS84")
            .put("observer", new JSONObject().put("kind", "localDevice").put("identityHex", "local")
                .put("lxmfDestinationHex", "destination"))
            .put("lat", 0).put("lon", 0).put("updatedAtMs", NOW - 60_000).put("ageMs", 60_000)
            .put("horizontalAccuracyM", 12.5);
    }

    static JSONObject marker(String id, double lat, double lon) throws Exception {
        return new JSONObject().put("object_destination_hash", id).put("name", "ÉQUIPE 救援")
            .put("symbol", "hostile").put("notes", "must not escape")
            .put("position", new JSONObject().put("lat", lat).put("lon", lon))
            .put("distanceM", 1).put("updated_at", "2026-10-20T00:00:00Z");
    }

    static JSONObject nearby(JSONArray markers) throws Exception {
        return new JSONObject().put("available", true).put("radiusM", 500).put("items", markers);
    }

    static JSONObject snapshot(JSONArray markers) throws Exception {
        return RadarSnapshot.build(observer(), nearby(markers), observer(), "request-1", "public-session", 1, NOW);
    }

    @Test public void derivesGeometryAndPreservesSourceAgesAndNatoColor() throws Exception {
        final JSONObject body = snapshot(new JSONArray().put(marker("east", 0, .001)).put(marker("center", 0, 0)));
        final JSONArray markers = body.getJSONArray("markers");
        assertEquals("center", markers.getJSONObject(0).getString("id"));
        assertTrue(markers.getJSONObject(0).isNull("bearingDegTrue"));
        final JSONObject east = markers.getJSONObject(1);
        assertEquals(111.195, east.getDouble("distanceM"), .01);
        assertEquals(90, east.getDouble("bearingDegTrue"), .001);
        assertEquals(0xff303b, east.getInt("color"));
        assertEquals("ÉQUIPE 救援", east.getString("label"));
        assertFalse(east.has("notes"));
        assertFalse(east.has("position"));
        assertEquals(NOW - 60_000, body.getJSONObject("observer").getLong("positionUpdatedAtUnixMs"));
        assertEquals(12.5, body.getJSONObject("observer").getDouble("accuracyM"), 0);
        assertEquals(java.time.Instant.parse("2026-10-20T00:00:00Z").toEpochMilli(), east.getLong("markerUpdatedAtUnixMs"));
    }

    @Test public void canonicalNatoSymbolsKeepAffiliationAndGenericSymbolsDoNotImplyIt() throws Exception {
        final String[] symbols = { "friendly", "hostile", "neutral", "unknown", "marker", "fire", "new-hostile-icon" };
        final int[] colors = { 0x007aff, 0xff303b, 0x00d43b, 0xffe600, 0x64beff, 0x64beff, 0x64beff };
        for (int i = 0; i < symbols.length; i++) {
            final JSONObject item = marker("nato", 0, .001).put("symbol", symbols[i]);
            assertEquals(symbols[i], colors[i], snapshot(new JSONArray().put(item)).getJSONArray("markers").getJSONObject(0).getInt("color"));
        }
        final JSONObject generic = marker("generic", 0, .001).put("symbol", JSONObject.NULL)
            .put("type", "hostile").put("name", "hostile").put("category", "nato");
        assertEquals(0x64beff, snapshot(new JSONArray().put(generic)).getJSONArray("markers").getJSONObject(0).getInt("color"));
    }

    @Test public void unknownTimesStayNullAndOutOfRangePinsAreFiltered() throws Exception {
        final JSONObject pin = marker("unknown", .001, 0).put("updated_at", "unknown");
        final JSONArray markers = snapshot(new JSONArray().put(pin).put(marker("far", 1, 1))).getJSONArray("markers");
        assertEquals(1, markers.length());
        assertTrue(markers.getJSONObject(0).isNull("markerUpdatedAtUnixMs"));
        assertEquals(0, markers.getJSONObject(0).getDouble("bearingDegTrue"), .001);
    }

    @Test public void rejectsObserverMovementIdentityChangeAndInvalidCoordinates() throws Exception {
        for (String field : new String[] { "lat", "lon", "updatedAtMs" }) {
            final JSONObject after = observer().put(field, 1);
            assertThrows(RadarFailure.class, () -> RadarSnapshot.build(observer(), nearby(new JSONArray()), after, "r", "s", 1, NOW));
        }
        final JSONObject after = observer();
        after.getJSONObject("observer").put("identityHex", "other");
        assertThrows(RadarFailure.class, () -> RadarSnapshot.requireSameObserver(observer(), after));
        assertThrows(Exception.class, () -> snapshot(new JSONArray().put(marker("invalid", 91, 0))));
        assertThrows(Exception.class, () -> snapshot(new JSONArray().put(marker("string", 0, 0)
            .put("position", new JSONObject().put("lat", "0").put("lon", 0)))));
    }

    @Test public void fiftyUnicodeRecordsAreBoundedAndSourceLimitedWithStableSorting() throws Exception {
        final JSONArray input = new JSONArray();
        final String[] symbols = { "friendly", "hostile", "neutral", "unknown", "marker" };
        for (int i = 49; i >= 0; i--) input.put(marker(String.format(java.util.Locale.ROOT, "id-%02d", i), .001, 0)
            .put("symbol", symbols[i % symbols.length]).put("name", "救援🚒".repeat(20)));
        final JSONObject body = snapshot(input);
        assertEquals(50, body.getJSONArray("markers").length());
        assertTrue(body.getBoolean("truncated"));
        assertEquals("id-00", body.getJSONArray("markers").getJSONObject(0).getString("id"));
        assertTrue(body.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8).length < 65_536);
        final String label = body.getJSONArray("markers").getJSONObject(0).getString("label");
        assertEquals(32, label.codePointCount(0, label.length()));
        final String output = System.getProperty("radar.contract.output");
        if (output != null) {
            final java.nio.file.Path path = java.nio.file.Path.of(output);
            java.nio.file.Files.createDirectories(path.getParent());
            java.nio.file.Files.write(path, body.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8));
        }
        input.put(marker("too-many", 0, 0));
        assertThrows(RadarFailure.class, () -> snapshot(input));
    }

    @Test public void unavailablePositionCannotLeakNearbyMarkers() throws Exception {
        final JSONObject source = observer().put("available", false);
        final JSONObject empty = nearby(new JSONArray()).put("available", false);
        final JSONObject body = RadarSnapshot.build(source, empty, source, "r", "s", 1, NOW);
        assertFalse(body.getJSONObject("observer").getBoolean("positionValid"));
        assertTrue(body.getJSONObject("observer").isNull("positionUpdatedAtUnixMs"));
        assertTrue(body.getJSONObject("observer").isNull("accuracyM"));
        empty.getJSONArray("items").put(marker("leak", 0, 0));
        assertThrows(RadarFailure.class, () -> RadarSnapshot.build(source, empty, source, "r", "s", 2, NOW));
    }

    @Test public void accuracyRemainsUnknownWhenMissingAndChangesInvalidateObserverPairing() throws Exception {
        for (Object value : new Object[] { JSONObject.NULL, 0, 25.75 }) {
            final JSONObject source = observer().put("horizontalAccuracyM", value);
            final JSONObject result = RadarSnapshot.build(source, nearby(new JSONArray()), source, "r", "s", 1, NOW);
            assertEquals(value == JSONObject.NULL, result.getJSONObject("observer").isNull("accuracyM"));
        }
        final JSONObject source = observer();
        source.remove("horizontalAccuracyM");
        assertTrue(RadarSnapshot.build(source, nearby(new JSONArray()), source, "r", "s", 1, NOW)
            .getJSONObject("observer").isNull("accuracyM"));
        for (Object value : new Object[] { -1, "12" }) {
            final JSONObject invalid = observer().put("horizontalAccuracyM", value);
            assertThrows(RadarFailure.class, () -> RadarSnapshot.build(invalid, nearby(new JSONArray()), invalid, "r", "s", 1, NOW));
        }
        assertThrows(RadarFailure.class, () -> RadarSnapshot.requireSameObserver(observer(), observer().put("horizontalAccuracyM", 30)));
    }

    @Test public void geodesyHandlesDateLineNearPolesAndAntipodesWithoutInvalidDerivedValues() {
        assertEquals(222.39016, RadarSnapshot.distance(0, 179.999, 0, -179.999), .01);
        assertEquals(90, RadarSnapshot.bearing(0, 179.999, 0, -179.999), .001);
        assertEquals(270, RadarSnapshot.bearing(0, -179.999, 0, 179.999), .001);
        assertTrue(RadarSnapshot.distance(89.999, 179.999, 89.999, -179.999) < .5);
        assertEquals(20_015_114.44, RadarSnapshot.distance(0, 0, 0, 180), .1);
        assertEquals(0, RadarSnapshot.distance(90, -180, 90, 180), .000001);
    }
}
