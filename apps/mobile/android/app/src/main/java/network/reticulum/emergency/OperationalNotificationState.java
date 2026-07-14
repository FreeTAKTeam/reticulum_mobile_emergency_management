package network.reticulum.emergency;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.util.HashSet;
import java.util.Locale;
import java.util.Set;

final class OperationalNotificationState {
    private final Set<String> eamKeys = new HashSet<>();
    private final Set<String> eventKeys = new HashSet<>();
    private final Set<String> checklistKeys = new HashSet<>();
    private final Set<String> messageIds = new HashSet<>();
    private final Set<String> missionPacketKeys = new HashSet<>();

    synchronized void prime() {
        messageIds.clear();
        missionPacketKeys.clear();
        primeEamKeys();
        primeEventKeys();
        primeChecklistKeys();
    }

    synchronized boolean markEam(String key) {
        return eamKeys.add(key);
    }

    synchronized boolean markEvent(String key) {
        return eventKeys.add(key);
    }

    synchronized boolean markChecklist(String key) {
        return checklistKeys.add(key);
    }

    synchronized boolean markMessage(String messageId) {
        return messageIds.add(messageId);
    }

    synchronized boolean markMissionPacket(String key) {
        return missionPacketKeys.add(key);
    }

    String checklistKey(JSONObject item) {
        final String uid = item.optString("uid", "").trim();
        final String stamp = latestChecklistStamp(item);
        return uid.isEmpty() || stamp.isEmpty()
            ? ""
            : uid.toLowerCase(Locale.US) + ":" + stamp;
    }

    String optStringAny(JSONObject item, String camelKey, String snakeKey) {
        if (item == null) {
            return "";
        }
        return item.optString(camelKey, item.optString(snakeKey, ""));
    }

    int optIntAny(JSONObject item, String camelKey, String snakeKey, int fallback) {
        if (item == null) {
            return fallback;
        }
        return item.optInt(camelKey, item.optInt(snakeKey, fallback));
    }

    private void primeEamKeys() {
        eamKeys.clear();
        try {
            final JSONObject root = new JSONObject(nonEmptyJson(ReticulumBridge.getEamsJson(), "{\"items\":[]}"));
            final JSONArray items = root.optJSONArray("items");
            if (items == null) {
                return;
            }
            for (int index = 0; index < items.length(); index += 1) {
                final JSONObject item = items.optJSONObject(index);
                if (item == null || item.has("deletedAt")) {
                    continue;
                }
                final String callsign = item.optString("callsign", "").trim();
                final long updatedAt = item.optLong("updatedAt", 0L);
                if (!callsign.isEmpty() && updatedAt > 0L) {
                    eamKeys.add(callsign.toLowerCase(Locale.US) + ":" + updatedAt);
                }
            }
        } catch (JSONException ignored) {
        }
    }

    private void primeEventKeys() {
        eventKeys.clear();
        try {
            final JSONObject root = new JSONObject(nonEmptyJson(ReticulumBridge.getEventsJson(), "{\"items\":[]}"));
            final JSONArray items = root.optJSONArray("items");
            if (items == null) {
                return;
            }
            for (int index = 0; index < items.length(); index += 1) {
                final JSONObject item = items.optJSONObject(index);
                if (item == null || item.has("deletedAt") || item.has("deleted_at")) {
                    continue;
                }
                final JSONObject args = item.optJSONObject("args");
                final String uid = item.optString(
                    "uid",
                    args == null ? "" : args.optString("entry_uid", "")
                ).trim();
                final long updatedAt = item.optLong("updatedAt", 0L);
                if (!uid.isEmpty() && updatedAt > 0L) {
                    eventKeys.add(uid.toLowerCase(Locale.US) + ":" + updatedAt);
                }
            }
        } catch (JSONException ignored) {
        }
    }

    private void primeChecklistKeys() {
        checklistKeys.clear();
        try {
            final JSONObject root = new JSONObject(nonEmptyJson(
                ReticulumBridge.getChecklistsJson("{\"sortBy\":\"updated_at_desc\"}"),
                "{\"items\":[]}"
            ));
            final JSONArray items = root.optJSONArray("items");
            if (items == null) {
                return;
            }
            for (int index = 0; index < items.length(); index += 1) {
                final JSONObject item = items.optJSONObject(index);
                if (item == null || item.has("deletedAt") || item.has("deleted_at")) {
                    continue;
                }
                final String key = checklistKey(item);
                if (!key.isEmpty()) {
                    checklistKeys.add(key);
                }
            }
        } catch (JSONException ignored) {
        }
    }

    private String latestChecklistStamp(JSONObject item) {
        String latest = "";
        for (String key : new String[] {"updatedAt", "updated_at", "uploadedAt", "uploaded_at"}) {
            final String value = item.optString(key, "").trim();
            if (!value.isEmpty() && value.compareTo(latest) > 0) {
                latest = value;
            }
        }
        return latest;
    }

    private String nonEmptyJson(String raw, String fallback) {
        return raw == null || raw.trim().isEmpty() ? fallback : raw;
    }
}
