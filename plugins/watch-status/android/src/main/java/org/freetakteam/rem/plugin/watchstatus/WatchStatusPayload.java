package org.freetakteam.rem.plugin.watchstatus;

import java.util.Locale;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

final class WatchStatusPayload {
    private WatchStatusPayload() {}

    static String build(JSONObject snapshot, long nowMs) throws JSONException {
        final JSONObject status = object(snapshot, "status");
        final JSONObject summary = object(snapshot, "operationalSummary");
        final JSONObject readiness = object(snapshot, "eamReadiness");
        final JSONObject event = snapshot == null ? null : snapshot.optJSONObject("latestEvent");
        final JSONObject telemetry = snapshot == null ? null : snapshot.optJSONObject("latestPosition");
        final boolean running = status.optBoolean("running", summary.optBoolean("running", false));
        final String runtimeError = first(status.optString("lastError", ""), status.optString("last_error", ""));
        final String operatorName = first(status.optString("name", ""), status.optString("operatorName", ""), "REM");
        final String connection = !runtimeError.isEmpty() ? "ERROR" : running ? "CONNECTED" : "OFFLINE";
        final String operatorEam = operatorBand(readiness, operatorName);
        final String teamStatus = teamBand(readiness, operatorEam);
        final JSONObject latestEvent = event == null ? null : event(event);
        final long lastSync = positive(summary, "updatedAtMs", snapshot == null ? nowMs : snapshot.optLong("capturedAtMs", nowMs));
        final String priority = priority(connection, teamStatus, latestEvent);

        final JSONObject payload = new JSONObject()
            .put("type", "rem.watch.status")
            .put("version", 1)
            .put("connection_state", connection)
            .put("operator_name", operatorName)
            .put("operator_status", "ERROR".equals(connection) ? "ERROR" : running ? "ACTIVE" : "STOPPED")
            .put("operator_eam", operatorEam)
            .put("team", first(status.optString("team", ""), "REM"))
            .put("team_status", teamStatus)
            .put("last_sync_epoch_ms", lastSync)
            .put("last_sync_age_seconds", Math.max(0L, (nowMs - lastSync) / 1_000L))
            .put("active_events", summary.optInt("eventCount", latestEvent == null ? 0 : 1))
            .put("highest_priority", priority)
            .put("alert_state", "ERROR".equals(connection) ? "ERROR" : "EMERGENCY".equals(priority) ? "ALERT" : "NORMAL");
        if (latestEvent != null) payload.put("latest_event", latestEvent);
        final JSONObject position = position(telemetry);
        if (position != null) payload.put("position", position);
        return payload.toString();
    }

    static String error(String message, long nowMs) {
        try {
            return build(new JSONObject()
                .put("capturedAtMs", nowMs)
                .put("status", new JSONObject().put("running", false).put("lastError", first(message, "Snapshot unavailable"))), nowMs);
        } catch (JSONException error) {
            android.util.Log.w("REM.WatchStatusPayload", "Unable to encode the fallback watch status", error);
            return "{\"type\":\"rem.watch.status\",\"version\":1,\"connection_state\":\"ERROR\",\"operator_name\":\"REM\",\"operator_status\":\"ERROR\",\"operator_eam\":\"UNKNOWN\",\"team\":\"REM\",\"team_status\":\"UNKNOWN\",\"last_sync_epoch_ms\":0,\"last_sync_age_seconds\":0,\"active_events\":0,\"highest_priority\":\"ERROR\",\"alert_state\":\"ERROR\"}";
        }
    }

    private static JSONObject object(JSONObject parent, String key) {
        final JSONObject value = parent == null ? null : parent.optJSONObject(key);
        return value == null ? new JSONObject() : value;
    }

    private static String first(String... values) {
        for (String value : values) if (value != null && !value.trim().isEmpty()) return value.trim();
        return "";
    }

    private static long positive(JSONObject value, String key, long fallback) {
        final long result = value.optLong(key, fallback);
        return result > 0L ? result : fallback;
    }

    private static String operatorBand(JSONObject readiness, String operatorName) {
        final JSONArray messages = readiness.optJSONArray("messages");
        if (messages == null) return "UNKNOWN";
        for (int index = 0; index < messages.length(); index++) {
            final JSONObject message = messages.optJSONObject(index);
            if (message != null && message.optString("callsign").trim().equalsIgnoreCase(operatorName)) {
                return band(message.optString("overallBand", message.optString("overall_band", "UNKNOWN")));
            }
        }
        return "UNKNOWN";
    }

    private static String teamBand(JSONObject readiness, String operatorBand) {
        String selected = rank(operatorBand) > 0 ? band(operatorBand) : "UNKNOWN";
        int selectedRank = rank(selected);
        final JSONArray metrics = readiness.optJSONArray("statusMetrics");
        if (metrics == null) return selected;
        for (int index = 0; index < metrics.length(); index++) {
            final JSONObject metric = metrics.optJSONObject(index);
            final String candidate = metric == null ? "UNKNOWN" : band(metric.optString("band", ""));
            if (rank(candidate) > selectedRank) { selected = candidate; selectedRank = rank(candidate); }
        }
        return selected;
    }

    private static JSONObject event(JSONObject record) throws JSONException {
        if (record.optLong("deletedAtMs", record.optLong("deleted_at_ms", 0L)) > 0L) return null;
        final JSONObject args = record.optJSONObject("args") == null ? new JSONObject() : record.optJSONObject("args");
        final JSONObject source = record.optJSONObject("source") == null ? new JSONObject() : record.optJSONObject("source");
        final JSONArray keywords = args.optJSONArray("keywords");
        final String category = keywords != null && keywords.length() > 0
            ? String.valueOf(keywords.opt(0)).toUpperCase(Locale.US)
            : category(record.optString("command_type", record.optString("type", "EVENT")));
        final String title = first(args.optString("content", ""), record.optString("title", ""), "Event");
        return new JSONObject()
            .put("severity", eventSeverity(category, title))
            .put("category", category)
            .put("title", title)
            .put("source", first(source.optString("display_name", ""), args.optString("callsign", ""), source.optString("rns_identity", ""), "REM"))
            .put("time", first(args.optString("server_time", ""), args.optString("serverTime", ""), record.optString("timestamp", ""), args.optString("client_time", "")));
    }

    private static JSONObject position(JSONObject item) throws JSONException {
        if (item == null) return null;
        final JSONObject result = new JSONObject();
        if (item.has("lat")) result.put("lat", item.optDouble("lat"));
        if (item.has("lon")) result.put("lon", item.optDouble("lon"));
        final String mgrs = first(item.optString("mgrs", ""), item.optString("grid", ""));
        if (!mgrs.isEmpty()) result.put("mgrs", mgrs);
        return result.length() == 0 ? null : result;
    }

    private static String priority(String connection, String team, JSONObject event) {
        if ("ERROR".equals(connection)) return "ERROR";
        if (event != null) return "EMERGENCY".equals(event.optString("severity").toUpperCase(Locale.US)) ? "EMERGENCY" : "HIGH";
        return rank(team) >= 3 ? "EMERGENCY" : rank(team) == 2 ? "HIGH" : "NORMAL";
    }

    private static String eventSeverity(String category, String title) {
        final String value = (category + " " + title).toUpperCase(Locale.US);
        return value.contains("SOS") || value.contains("EMERGENCY") || value.contains("MAYDAY") ? "EMERGENCY" : "HIGH";
    }

    private static String category(String value) {
        final String selected = first(value, "EVENT");
        final int separator = selected.indexOf('.');
        return (separator >= 0 ? selected.substring(separator + 1) : selected).toUpperCase(Locale.US);
    }

    private static String band(String value) {
        return switch (first(value, "UNKNOWN").toLowerCase(Locale.US)) {
            case "red" -> "Red";
            case "orange" -> "Orange";
            case "yellow" -> "Yellow";
            case "green" -> "Green";
            default -> "UNKNOWN";
        };
    }

    private static int rank(String value) {
        final String normalized = String.valueOf(value).toLowerCase(Locale.US);
        if (normalized.equals("red") || normalized.equals("emergency") || normalized.equals("error")) return 3;
        if (normalized.equals("orange") || normalized.equals("yellow") || normalized.equals("high") || normalized.equals("urgent")) return 2;
        if (normalized.equals("green") || normalized.equals("normal")) return 1;
        return 0;
    }
}
