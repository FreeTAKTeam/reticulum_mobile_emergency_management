package org.freetakteam.rem.plugin.mapfixture;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.HashSet;
import network.reticulum.emergency.plugin.api.PluginProtocol;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Reference consumer of the REM plugin map API, free of Android types so it runs in JVM tests.
 * On start it reads the symbol catalog and the observer fix, then creates one marker a fixed
 * distance north of the observer (as a rangefinder would). When the operator shares objects
 * with it, it reads exactly those objects and follows their session events.
 */
final class MapFixtureFlow {
    static final double TARGET_DISTANCE_M = 100.0;
    private static final double METERS_PER_DEGREE_LAT = 111_320.0;

    interface RequestIds {
        String next();
    }

    private final RequestIds requestIds;
    private final Map<String, String> pendingOperations = new HashMap<>();
    private final Map<String, Set<String>> sessionSelections = new HashMap<>();
    private final Map<String, Map<String, JSONObject>> sessionObjects = new HashMap<>();
    private final List<String> violations = new ArrayList<>();
    private int apiMinor = -1;
    private String symbol = "";
    private String createdMarkerId = "";

    MapFixtureFlow(RequestIds requestIds) {
        this.requestIds = requestIds;
    }

    /** Returns the first requests, or none when REM negotiated an API older than 1.3. */
    List<String> start(String sessionJson) throws JSONException {
        apiMinor = new JSONObject(sessionJson).optInt("apiMinor", -1);
        if (apiMinor < PluginProtocol.MAP_SESSION_API_MINOR) {
            return Collections.emptyList();
        }
        final List<String> requests = new ArrayList<>();
        requests.add(request("map.symbols.list", null));
        return requests;
    }

    void stop() {
        apiMinor = -1;
        pendingOperations.clear();
        sessionSelections.clear();
        sessionObjects.clear();
    }

    List<String> onHostResponse(String responseJson) throws JSONException {
        PluginProtocol.requireJsonSize(responseJson, "Host response");
        final JSONObject response = new JSONObject(responseJson);
        final String operation = pendingOperations.remove(response.optString("requestId", ""));
        if (operation == null || !response.optBoolean("ok")) {
            return Collections.emptyList();
        }
        final JSONObject result = response.optJSONObject("result");
        if (result == null) {
            return Collections.emptyList();
        }
        switch (operation) {
            case "map.symbols.list":
                symbol = pickSymbol(result.optJSONArray("items"));
                return List.of(request("map.observer.snapshot", null));
            case "map.observer.snapshot":
                return createTargetFrom(result);
            case "map.markers.create":
                final JSONObject marker = result.optJSONObject("marker");
                createdMarkerId = marker == null ? "" : marker.optString("object_destination_hash", "");
                return Collections.emptyList();
            case "map.session.objects.get":
                acceptSessionObjects(result);
                return Collections.emptyList();
            default:
                return Collections.emptyList();
        }
    }

    List<String> onHostEvent(String eventJson) throws JSONException {
        PluginProtocol.requireJsonSize(eventJson, "Host event");
        final JSONObject event = new JSONObject(eventJson);
        final JSONObject payload = event.optJSONObject("payload");
        if (payload == null || apiMinor < PluginProtocol.MAP_SESSION_API_MINOR) {
            return Collections.emptyList();
        }
        final String sessionId = payload.optString("sessionId", "");
        switch (event.optString("event", "")) {
            case PluginProtocol.EVENT_MAP_SESSION_STARTED:
                final Set<String> selected = new HashSet<>();
                final JSONArray ids = payload.optJSONArray("objectDestinationHashes");
                for (int index = 0; ids != null && index < ids.length(); index++) {
                    selected.add(ids.getString(index));
                }
                sessionSelections.put(sessionId, selected);
                sessionObjects.put(sessionId, new LinkedHashMap<>());
                return List.of(request("map.session.objects.get", new JSONObject().put("sessionId", sessionId)));
            case PluginProtocol.EVENT_MAP_SESSION_OBJECT_UPDATED:
                if (sessionSelections.containsKey(sessionId)) {
                    acceptObject(sessionId, payload.optJSONObject("object"));
                }
                return Collections.emptyList();
            case PluginProtocol.EVENT_MAP_SESSION_OBJECT_REMOVED:
                final String removed = payload.optString("objectDestinationHash", "");
                if (sessionSelections.containsKey(sessionId)) {
                    sessionSelections.get(sessionId).remove(removed);
                    sessionObjects.get(sessionId).remove(removed);
                }
                return Collections.emptyList();
            case PluginProtocol.EVENT_MAP_SESSION_STOPPED:
                sessionSelections.remove(sessionId);
                sessionObjects.remove(sessionId);
                return Collections.emptyList();
            default:
                return Collections.emptyList();
        }
    }

    String createdMarkerId() {
        return createdMarkerId;
    }

    Map<String, JSONObject> selectedObjects(String sessionId) {
        final Map<String, JSONObject> objects = sessionObjects.get(sessionId);
        return objects == null ? Collections.emptyMap() : Collections.unmodifiableMap(objects);
    }

    Set<String> activeSessions() {
        return Collections.unmodifiableSet(sessionSelections.keySet());
    }

    /** Objects REM delivered outside the session selection; the contract says this stays empty. */
    List<String> violations() {
        return Collections.unmodifiableList(violations);
    }

    private List<String> createTargetFrom(JSONObject snapshot) throws JSONException {
        final JSONObject observer = snapshot.optJSONObject("observer");
        if (!snapshot.optBoolean("available")
            || observer == null
            || !"localDevice".equals(observer.optString("kind"))
            || symbol.isEmpty()) {
            return Collections.emptyList();
        }
        final double lat = snapshot.getDouble("lat") + TARGET_DISTANCE_M / METERS_PER_DEGREE_LAT;
        final JSONObject payload = new JSONObject()
            .put("idempotencyKey", "fixture-target-" + snapshot.optLong("updatedAtMs"))
            .put("symbol", symbol)
            .put("name", "Fixture target")
            .put("notes", "100 m north of the REM observer fix")
            .put("lat", Math.min(90.0, lat))
            .put("lon", snapshot.getDouble("lon"));
        return List.of(request("map.markers.create", payload));
    }

    private void acceptSessionObjects(JSONObject result) {
        final JSONObject session = result.optJSONObject("session");
        final String sessionId = session == null ? "" : session.optString("sessionId", "");
        final JSONArray objects = result.optJSONArray("objects");
        if (!sessionSelections.containsKey(sessionId) || objects == null) {
            return;
        }
        for (int index = 0; index < objects.length(); index++) {
            acceptObject(sessionId, objects.optJSONObject(index));
        }
    }

    private void acceptObject(String sessionId, JSONObject object) {
        if (object == null) {
            return;
        }
        final String id = object.optString("object_destination_hash", "");
        if (!sessionSelections.get(sessionId).contains(id)) {
            violations.add(id);
            return;
        }
        sessionObjects.get(sessionId).put(id, object);
    }

    private static String pickSymbol(JSONArray items) {
        String first = "";
        for (int index = 0; items != null && index < items.length(); index++) {
            final JSONObject item = items.optJSONObject(index);
            final String id = item == null ? "" : item.optString("id", "");
            if ("hostile".equals(id)) {
                return id;
            }
            if (first.isEmpty()) {
                first = id;
            }
        }
        return first;
    }

    private String request(String operation, JSONObject payload) throws JSONException {
        final String requestId = requestIds.next();
        pendingOperations.put(requestId, operation);
        return PluginProtocol.request(requestId, operation, payload);
    }
}
