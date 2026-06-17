package network.reticulum.emergency;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.util.Locale;

final class RemWatchStatusPayload {
    private RemWatchStatusPayload() {
    }

    static String build(
        String statusJson,
        String operationalSummaryJson,
        String eamReadinessJson,
        String eventsJson,
        String telemetryPositionsJson,
        long nowMs
    ) throws JSONException {
        final JSONObject status = parseObject(statusJson);
        final JSONObject summary = parseObject(operationalSummaryJson);
        final JSONObject readiness = parseObject(eamReadinessJson);
        final JSONArray events = itemsArray(parseObject(eventsJson));
        final JSONObject telemetry = parseObject(telemetryPositionsJson);

        final boolean running = status.optBoolean("running", summary.optBoolean("running", false));
        final String runtimeError = firstNonBlank(
            status.optString("lastError", ""),
            status.optString("last_error", "")
        );
        final String operatorName = firstNonBlank(
            status.optString("name", ""),
            status.optString("operatorName", ""),
            status.optString("displayName", ""),
            "REM"
        );
        final String connectionState = !runtimeError.isEmpty()
            ? "ERROR"
            : running
                ? "CONNECTED"
                : "OFFLINE";
        final String operatorStatus = "ERROR".equals(connectionState)
            ? "ERROR"
            : running ? "ACTIVE" : "STOPPED";
        final String operatorEam = findOperatorReadinessBand(readiness, operatorName);
        final String teamStatus = teamStatus(readiness, operatorEam);
        final JSONObject latestEvent = latestEvent(events);
        final int activeEvents = Math.max(summary.optInt("eventCount", events.length()), events.length());
        final long lastSyncMs = positiveLong(summary, "updatedAtMs", summary.optLong("updated_at_ms", nowMs));
        final long ageSeconds = Math.max(0L, (nowMs - (lastSyncMs > 0L ? lastSyncMs : nowMs)) / 1_000L);
        final String highestPriority = highestPriority(connectionState, teamStatus, latestEvent);

        final JSONObject payload = new JSONObject();
        payload.put("type", "rem.watch.status");
        payload.put("version", 1);
        payload.put("connection_state", connectionState);
        payload.put("operator_name", operatorName);
        payload.put("operator_status", operatorStatus);
        payload.put("operator_eam", operatorEam);
        payload.put("team", firstNonBlank(status.optString("team", ""), "REM"));
        payload.put("team_status", teamStatus);
        payload.put("last_sync_epoch_ms", lastSyncMs > 0L ? lastSyncMs : nowMs);
        payload.put("last_sync_age_seconds", ageSeconds);
        payload.put("active_events", activeEvents);
        payload.put("highest_priority", highestPriority);
        payload.put("alert_state", "ERROR".equals(connectionState) ? "ERROR" : "EMERGENCY".equals(highestPriority) ? "ALERT" : "NORMAL");

        if (latestEvent != null) {
            payload.put("latest_event", latestEvent);
        }

        final JSONObject position = latestPosition(telemetry);
        if (position != null) {
            payload.put("position", position);
        }

        return payload.toString();
    }

    private static JSONObject parseObject(String raw) {
        if (raw == null || raw.trim().isEmpty()) {
            return new JSONObject();
        }
        try {
            return new JSONObject(raw);
        } catch (JSONException ex) {
            return new JSONObject();
        }
    }

    private static JSONArray itemsArray(JSONObject object) {
        final JSONArray items = object.optJSONArray("items");
        return items == null ? new JSONArray() : items;
    }

    private static String firstNonBlank(String... values) {
        for (String value : values) {
            if (value != null && !value.trim().isEmpty()) {
                return value.trim();
            }
        }
        return "";
    }

    private static long positiveLong(JSONObject object, String key, long fallback) {
        final long value = object.optLong(key, fallback);
        return value > 0L ? value : fallback;
    }

    private static String findOperatorReadinessBand(JSONObject readiness, String operatorName) {
        final JSONArray messages = readiness.optJSONArray("messages");
        if (messages == null) {
            return "UNKNOWN";
        }
        for (int index = 0; index < messages.length(); index += 1) {
            final JSONObject message = messages.optJSONObject(index);
            if (message == null) {
                continue;
            }
            final String callsign = message.optString("callsign", "").trim();
            if (!callsign.equalsIgnoreCase(operatorName)) {
                continue;
            }
            return normalizeBand(message.optString("overallBand", message.optString("overall_band", "UNKNOWN")));
        }
        return "UNKNOWN";
    }

    private static String teamStatus(JSONObject readiness, String operatorEam) {
        String selected = "UNKNOWN";
        int selectedRank = severityRank(operatorEam);
        if (selectedRank > 0) {
            selected = normalizeBand(operatorEam);
        }

        final JSONArray metrics = readiness.optJSONArray("statusMetrics");
        if (metrics == null) {
            return selected;
        }
        for (int index = 0; index < metrics.length(); index += 1) {
            final JSONObject metric = metrics.optJSONObject(index);
            if (metric == null) {
                continue;
            }
            final String band = normalizeBand(metric.optString("band", ""));
            final int rank = severityRank(band);
            if (rank > selectedRank) {
                selected = band;
                selectedRank = rank;
            }
        }
        return selected;
    }

    private static JSONObject latestEvent(JSONArray events) throws JSONException {
        for (int index = 0; index < events.length(); index += 1) {
            final JSONObject record = events.optJSONObject(index);
            if (record == null || record.optLong("deletedAtMs", record.optLong("deleted_at_ms", 0L)) > 0L) {
                continue;
            }

            final JSONObject args = record.optJSONObject("args") == null
                ? new JSONObject()
                : record.optJSONObject("args");
            final JSONObject source = record.optJSONObject("source") == null
                ? new JSONObject()
                : record.optJSONObject("source");
            final JSONArray keywords = args.optJSONArray("keywords");
            final String category = keywords != null && keywords.length() > 0
                ? String.valueOf(keywords.opt(0)).toUpperCase(Locale.US)
                : normalizeCategory(record.optString("command_type", record.optString("type", "EVENT")));
            final String title = firstNonBlank(
                args.optString("content", ""),
                record.optString("title", ""),
                "Event"
            );
            final String eventSource = firstNonBlank(
                source.optString("display_name", ""),
                args.optString("callsign", ""),
                source.optString("rns_identity", ""),
                "REM"
            );
            final String time = firstNonBlank(
                args.optString("server_time", ""),
                args.optString("serverTime", ""),
                record.optString("timestamp", ""),
                args.optString("client_time", "")
            );

            final JSONObject latest = new JSONObject();
            latest.put("severity", eventSeverity(category, title));
            latest.put("category", category);
            latest.put("title", title);
            latest.put("source", eventSource);
            latest.put("time", time);
            return latest;
        }
        return null;
    }

    private static JSONObject latestPosition(JSONObject telemetry) throws JSONException {
        final JSONArray items = itemsArray(telemetry);
        if (items.length() == 0) {
            return null;
        }
        final JSONObject item = items.optJSONObject(0);
        if (item == null) {
            return null;
        }
        final JSONObject position = new JSONObject();
        if (item.has("lat")) {
            position.put("lat", item.optDouble("lat"));
        }
        if (item.has("lon")) {
            position.put("lon", item.optDouble("lon"));
        }
        final String mgrs = firstNonBlank(item.optString("mgrs", ""), item.optString("grid", ""));
        if (!mgrs.isEmpty()) {
            position.put("mgrs", mgrs);
        }
        return position.length() == 0 ? null : position;
    }

    private static String highestPriority(String connectionState, String teamStatus, JSONObject latestEvent) {
        if ("ERROR".equals(connectionState)) {
            return "ERROR";
        }
        if (latestEvent != null) {
            final String severity = latestEvent.optString("severity", "HIGH").toUpperCase(Locale.US);
            if ("EMERGENCY".equals(severity)) {
                return "EMERGENCY";
            }
            return "HIGH";
        }
        final int rank = severityRank(teamStatus);
        if (rank >= 3) {
            return "EMERGENCY";
        }
        if (rank == 2) {
            return "HIGH";
        }
        return "NORMAL";
    }

    private static String eventSeverity(String category, String title) {
        final String combined = (category + " " + title).toUpperCase(Locale.US);
        if (combined.contains("SOS") || combined.contains("EMERGENCY") || combined.contains("MAYDAY")) {
            return "EMERGENCY";
        }
        return "HIGH";
    }

    private static String normalizeCategory(String value) {
        final String trimmed = firstNonBlank(value, "EVENT");
        final int separator = trimmed.indexOf('.');
        return (separator >= 0 ? trimmed.substring(separator + 1) : trimmed).toUpperCase(Locale.US);
    }

    private static String normalizeBand(String value) {
        final String normalized = firstNonBlank(value, "UNKNOWN").toLowerCase(Locale.US);
        if ("red".equals(normalized)) {
            return "Red";
        }
        if ("yellow".equals(normalized)) {
            return "Yellow";
        }
        if ("green".equals(normalized)) {
            return "Green";
        }
        return "UNKNOWN";
    }

    private static int severityRank(String value) {
        final String normalized = String.valueOf(value).toLowerCase(Locale.US);
        if ("red".equals(normalized) || "emergency".equals(normalized) || "error".equals(normalized)) {
            return 3;
        }
        if ("yellow".equals(normalized) || "high".equals(normalized) || "urgent".equals(normalized)) {
            return 2;
        }
        if ("green".equals(normalized) || "normal".equals(normalized)) {
            return 1;
        }
        return 0;
    }
}
