package network.reticulum.emergency;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;
import android.os.Build;
import android.os.Handler;
import android.util.Base64;

import androidx.core.app.NotificationCompat;
import androidx.core.app.NotificationManagerCompat;

import com.getcapacitor.JSObject;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.nio.charset.StandardCharsets;
import java.util.Locale;
import java.util.concurrent.ExecutorService;

final class ServiceNotificationController {
    interface ForegroundState {
        boolean isForeground();
    }

    interface StatusProvider {
        String statusJson();
    }

    static final String RUNTIME_CHANNEL_ID = "mesh-runtime";
    private static final String UPDATES_CHANNEL_ID = "operational-updates";
    private static final String SOS_CHANNEL_ID = "sos-emergency";
    private static final int SOS_NOTIFICATION_ID = 41002;
    private static final int BACKGROUND_NOTIFICATION_BASE_ID = 47000;

    private final Context context;
    private final Handler mainHandler;
    private final ExecutorService refreshExecutor;
    private final ForegroundState foregroundState;
    private final StatusProvider statusProvider;
    private final OperationalNotificationState state = new OperationalNotificationState();
    private int nextBackgroundNotificationId = BACKGROUND_NOTIFICATION_BASE_ID;

    ServiceNotificationController(
        Context context,
        Handler mainHandler,
        ExecutorService refreshExecutor,
        ForegroundState foregroundState,
        StatusProvider statusProvider
    ) {
        this.context = context;
        this.mainHandler = mainHandler;
        this.refreshExecutor = refreshExecutor;
        this.foregroundState = foregroundState;
        this.statusProvider = statusProvider;
    }

    void createChannels() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) {
            return;
        }
        final NotificationManager manager = context.getSystemService(NotificationManager.class);
        if (manager == null) {
            return;
        }
        final NotificationChannel runtimeChannel = new NotificationChannel(
            RUNTIME_CHANNEL_ID,
            "Mesh Runtime",
            NotificationManager.IMPORTANCE_LOW
        );
        runtimeChannel.setDescription("Foreground Reticulum mesh runtime");
        final NotificationChannel updatesChannel = new NotificationChannel(
            UPDATES_CHANNEL_ID,
            "Operational Updates",
            NotificationManager.IMPORTANCE_DEFAULT
        );
        updatesChannel.setDescription("Incoming mesh events, action messages, and chat");
        final NotificationChannel sosChannel = new NotificationChannel(
            SOS_CHANNEL_ID,
            "SOS Emergency",
            NotificationManager.IMPORTANCE_HIGH
        );
        sosChannel.setDescription("Urgent SOS alerts received over the mesh");
        manager.createNotificationChannel(runtimeChannel);
        manager.createNotificationChannel(updatesChannel);
        manager.createNotificationChannel(sosChannel);
    }

    void primeOperationalState() {
        state.prime();
    }

    void handleInboundUpdate(String eventName, JSObject payload) {
        if ("log".equals(eventName)) {
            maybeNotifyInboundMissionLog(payload);
            return;
        }
        if ("packetReceived".equals(eventName)) {
            maybeNotifyInboundMissionPacket(payload);
            return;
        }
        if ("messageReceived".equals(eventName) || "messageUpdated".equals(eventName)) {
            maybeNotifyInboundMessage(payload);
            return;
        }
        if (!"projectionInvalidated".equals(eventName)) {
            return;
        }
        notifyScope(payload.getString("scope", ""));
    }

    void handleSosAlert(JSObject payload, boolean postUserVisibleNotification) {
        final JSONObject nestedAlert = payload.optJSONObject("alert");
        final JSONObject alert = nestedAlert == null ? payload : nestedAlert;
        final boolean active = alert.optBoolean("active", true);
        if (!active) {
            NotificationManagerCompat.from(context).cancel(SOS_NOTIFICATION_ID);
            if (postUserVisibleNotification) {
                postBackgroundNotification("SOS cancelled", "The sender marked themselves safe.");
            }
            return;
        }
        if (!postUserVisibleNotification) {
            return;
        }
        final String source = alert.optString("sourceHex", "Unknown");
        final String body = truncate(alert.optString("bodyUtf8", "Emergency SOS alert"));
        postSosNotification("SOS EMERGENCY from " + source, body);
    }

    private void maybeNotifyInboundMissionPacket(JSObject payload) {
        final String fieldsBase64 = payload.getString("fieldsBase64", "");
        if (fieldsBase64.isEmpty()) {
            return;
        }
        final String fieldsText = decodeBase64Text(fieldsBase64);
        final String sourceHex = payload.getString("sourceHex", "").trim().toLowerCase(Locale.US);
        final String destinationHex = payload.getString("destinationHex", "").trim().toLowerCase(Locale.US);
        final String key = sourceHex + ":" + destinationHex + ":" + Integer.toHexString(fieldsBase64.hashCode());
        if (!state.markMissionPacket(key)) {
            return;
        }
        final String body = truncate(decodeBase64Text(payload.getString("bytesBase64", "")).trim());
        if (fieldsText.contains("mission.registry.eam.upsert")) {
            postBackgroundNotification("EAM from mesh", body.isEmpty() ? "Action Emergency Message updated" : body);
        } else if (fieldsText.contains("mission.registry.log_entry.upsert")) {
            postBackgroundNotification("Event from mesh", body.isEmpty() ? "Event updated" : body);
        } else if (fieldsText.contains("checklist.task.status.set")) {
            postBackgroundNotification("Checklist updated", body.isEmpty() ? "Checklist task status changed" : body);
        }
    }

    private void maybeNotifyInboundMissionLog(JSObject payload) {
        final String message = payload.getString("message", "");
        if (!message.contains("[lxmf][mission] received kind=command")) {
            return;
        }
        if (message.contains("name=mission.registry.eam.upsert")) {
            scheduleRefresh("Eams");
        } else if (message.contains("name=mission.registry.log_entry.upsert")) {
            scheduleRefresh("Events");
        } else if (message.contains("name=checklist.task.status.set")) {
            scheduleRefresh("Checklists");
        }
    }

    private void scheduleRefresh(String scope) {
        mainHandler.postDelayed(() -> refreshExecutor.execute(() -> {
            if (!foregroundState.isForeground()) {
                notifyScope(scope);
            }
        }), 1_500L);
    }

    private void notifyScope(String scope) {
        if ("Eams".equals(scope)) {
            maybeNotifyInboundEams();
        } else if ("Events".equals(scope)) {
            maybeNotifyInboundEvents();
        } else if ("Checklists".equals(scope)) {
            maybeNotifyInboundChecklists();
        }
    }

    private void maybeNotifyInboundMessage(JSObject payload) {
        final String direction = payload.getString("direction", "");
        final String messageId = payload.getString("messageIdHex", "").trim().toLowerCase();
        if (!"Inbound".equals(direction) || messageId.isEmpty() || !state.markMessage(messageId)) {
            return;
        }
        final String peer = payload.getString("sourceHex", payload.getString("destinationHex", "Unknown"));
        final String body = truncate(payload.getString("bodyUtf8", "(empty message)"));
        if (!isSosMessageBody(body)) {
            postBackgroundNotification("Message from " + peer, body);
        }
    }

    private boolean isSosMessageBody(String body) {
        final String normalized = body == null ? "" : body.trim().toLowerCase(Locale.US);
        return normalized.startsWith("sos!") || normalized.startsWith("sos cancelled");
    }

    private void maybeNotifyInboundEams() {
        try {
            final JSONObject root = new JSONObject(nonEmptyJson(ReticulumBridge.getEamsJson(), "{\"items\":[]}"));
            final JSONArray items = root.optJSONArray("items");
            if (items == null) {
                return;
            }
            final JSONObject status = new JSONObject(nonEmptyJson(statusProvider.statusJson(), "{}"));
            final String localIdentity = status.optString("identityHex", "").trim().toLowerCase();
            final String localAppDestination = status.optString("appDestinationHex", "").trim().toLowerCase();
            final String localName = status.optString("name", "").trim().toLowerCase();
            for (int index = 0; index < items.length(); index += 1) {
                final JSONObject item = items.optJSONObject(index);
                if (item == null || item.has("deletedAt")) {
                    continue;
                }
                final String callsign = item.optString("callsign", "").trim();
                final long updatedAt = item.optLong("updatedAt", 0L);
                if (callsign.isEmpty() || updatedAt <= 0L || !state.markEam(callsign.toLowerCase() + ":" + updatedAt)) {
                    continue;
                }
                final String teamMemberUid = item.optString("teamMemberUid", "").trim().toLowerCase();
                final JSONObject source = item.optJSONObject("source");
                final String sourceIdentity = source == null
                    ? ""
                    : source.optString("rnsIdentity", source.optString("rns_identity", "")).trim().toLowerCase(Locale.US);
                final String reportedBy = item.optString("reportedBy", "").trim().toLowerCase();
                if (
                    (!localAppDestination.isEmpty() && localAppDestination.equals(teamMemberUid))
                        || (!localIdentity.isEmpty() && localIdentity.equals(sourceIdentity))
                        || (!localName.isEmpty() && (localName.equals(reportedBy) || localName.equals(callsign.toLowerCase())))
                ) {
                    continue;
                }
                final String notes = item.optString("notes", "").trim();
                final String body = !notes.isEmpty()
                    ? truncate(notes)
                    : truncate(item.optString("groupName", "Team") + " status " + item.optString("overallStatus", "updated"));
                postBackgroundNotification("EAM from " + item.optString("reportedBy", callsign), body);
            }
        } catch (JSONException ignored) {
        }
    }

    private void maybeNotifyInboundEvents() {
        try {
            final JSONObject root = new JSONObject(nonEmptyJson(ReticulumBridge.getEventsJson(), "{\"items\":[]}"));
            final JSONArray items = root.optJSONArray("items");
            if (items == null) {
                return;
            }
            final JSONObject status = new JSONObject(nonEmptyJson(statusProvider.statusJson(), "{}"));
            final String localIdentity = status.optString("identityHex", "").trim().toLowerCase();
            final String localName = status.optString("name", "").trim().toLowerCase();
            for (int index = 0; index < items.length(); index += 1) {
                final JSONObject item = items.optJSONObject(index);
                final JSONObject args = item == null ? null : item.optJSONObject("args");
                final JSONObject source = item == null ? null : item.optJSONObject("source");
                if (args == null || source == null || item.has("deleted_at")) {
                    continue;
                }
                final String uid = args.optString("entry_uid", "").trim();
                final long updatedAt = item.optLong("updatedAt", 0L);
                if (uid.isEmpty() || updatedAt <= 0L || !state.markEvent(uid.toLowerCase() + ":" + updatedAt)) {
                    continue;
                }
                final String sourceIdentity = args.optString(
                    "sourceIdentity",
                    args.optString("source_identity", source.optString("rnsIdentity", source.optString("rns_identity", "")))
                ).trim().toLowerCase(Locale.US);
                final String sourceDisplayName = args.optString(
                    "sourceDisplayName",
                    args.optString("source_display_name", source.optString("displayName", source.optString("display_name", "")))
                ).trim().toLowerCase(Locale.US);
                final String callsign = args.optString("callsign", "").trim();
                if (
                    (!localIdentity.isEmpty() && localIdentity.equals(sourceIdentity))
                        || (!localName.isEmpty() && (localName.equals(sourceDisplayName) || localName.equals(callsign.toLowerCase())))
                ) {
                    continue;
                }
                postBackgroundNotification(
                    "Event from " + (callsign.isEmpty() ? "Unknown" : callsign),
                    truncate(args.optString("content", "Event updated"))
                );
            }
        } catch (JSONException ignored) {
        }
    }

    private void maybeNotifyInboundChecklists() {
        try {
            final JSONObject root = new JSONObject(nonEmptyJson(
                ReticulumBridge.getChecklistsJson("{\"sortBy\":\"updated_at_desc\"}"),
                "{\"items\":[]}"
            ));
            final JSONArray items = root.optJSONArray("items");
            if (items == null) {
                return;
            }
            final JSONObject status = new JSONObject(nonEmptyJson(statusProvider.statusJson(), "{}"));
            final String localIdentity = status.optString("identityHex", "").trim().toLowerCase(Locale.US);
            for (int index = 0; index < items.length(); index += 1) {
                final JSONObject item = items.optJSONObject(index);
                if (item == null || item.has("deletedAt") || item.has("deleted_at")) {
                    continue;
                }
                final String key = state.checklistKey(item);
                if (key.isEmpty() || !state.markChecklist(key)) {
                    continue;
                }
                final String changedBy = state.optStringAny(
                    item,
                    "lastChangedByTeamMemberRnsIdentity",
                    "last_changed_by_team_member_rns_identity"
                ).trim().toLowerCase(Locale.US);
                final String createdBy = state.optStringAny(
                    item,
                    "createdByTeamMemberRnsIdentity",
                    "created_by_team_member_rns_identity"
                ).trim().toLowerCase(Locale.US);
                if (
                    !localIdentity.isEmpty()
                        && (localIdentity.equals(changedBy) || (changedBy.isEmpty() && localIdentity.equals(createdBy)))
                ) {
                    continue;
                }
                final JSONObject counts = item.optJSONObject("counts");
                final int pendingCount = state.optIntAny(counts, "pendingCount", "pending_count", 0);
                final int completeCount = state.optIntAny(counts, "completeCount", "complete_count", 0);
                final int lateCount = state.optIntAny(counts, "lateCount", "late_count", 0);
                final JSONArray tasks = item.optJSONArray("tasks");
                final int taskCount = tasks == null ? 0 : tasks.length();
                final String lateSummary = lateCount > 0 ? ", " + lateCount + " late" : "";
                final String taskSummary = taskCount == 1 ? "1 task" : taskCount + " tasks";
                postBackgroundNotification(
                    "Checklist updated: " + item.optString("name", "Checklist"),
                    truncate(pendingCount + " pending, " + completeCount + " complete" + lateSummary + " across " + taskSummary)
                );
            }
        } catch (JSONException ignored) {
        }
    }

    private void postBackgroundNotification(String title, String body) {
        final int notificationId = nextNotificationId();
        final PendingIntent contentIntent = launchPendingIntent(notificationId);
        final Notification notification = new NotificationCompat.Builder(context, UPDATES_CHANNEL_ID)
            .setContentTitle(title)
            .setContentText(body)
            .setStyle(new NotificationCompat.BigTextStyle().bigText(body))
            .setSmallIcon(R.mipmap.ic_launcher)
            .setAutoCancel(true)
            .setOnlyAlertOnce(true)
            .setContentIntent(contentIntent)
            .build();
        NotificationManagerCompat.from(context).notify(notificationId, notification);
    }

    private void postSosNotification(String title, String body) {
        final PendingIntent contentIntent = launchPendingIntent(SOS_NOTIFICATION_ID);
        final Notification notification = new NotificationCompat.Builder(context, SOS_CHANNEL_ID)
            .setContentTitle(title)
            .setContentText(body)
            .setStyle(new NotificationCompat.BigTextStyle().bigText(body))
            .setSmallIcon(R.mipmap.ic_launcher)
            .setOngoing(true)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setCategory(NotificationCompat.CATEGORY_ALARM)
            .setContentIntent(contentIntent)
            .addAction(0, "Open Chat", contentIntent)
            .addAction(0, "View on Map", contentIntent)
            .build();
        NotificationManagerCompat.from(context).notify(SOS_NOTIFICATION_ID, notification);
    }

    private PendingIntent launchPendingIntent(int requestCode) {
        final Intent launchIntent = new Intent(context, MainActivity.class);
        launchIntent.addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP | Intent.FLAG_ACTIVITY_NEW_TASK);
        return PendingIntent.getActivity(
            context,
            requestCode,
            launchIntent,
            PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE
        );
    }

    private synchronized int nextNotificationId() {
        final int notificationId = nextBackgroundNotificationId;
        nextBackgroundNotificationId += 1;
        if (nextBackgroundNotificationId > BACKGROUND_NOTIFICATION_BASE_ID + 10_000) {
            nextBackgroundNotificationId = BACKGROUND_NOTIFICATION_BASE_ID;
        }
        return notificationId;
    }

    private String decodeBase64Text(String raw) {
        if (raw == null || raw.trim().isEmpty()) {
            return "";
        }
        try {
            return new String(Base64.decode(raw, Base64.DEFAULT), StandardCharsets.UTF_8);
        } catch (IllegalArgumentException ex) {
            return "";
        }
    }

    private String nonEmptyJson(String raw, String fallback) {
        return raw == null || raw.trim().isEmpty() ? fallback : raw;
    }

    private String truncate(String value) {
        if (value == null) {
            return "";
        }
        final String normalized = value.trim();
        return normalized.length() <= 160 ? normalized : normalized.substring(0, 157) + "...";
    }
}
