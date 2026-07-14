package network.reticulum.emergency;

import android.Manifest;
import android.content.Intent;

import com.getcapacitor.JSObject;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.CapacitorPlugin;
import com.getcapacitor.annotation.Permission;

@CapacitorPlugin(
    name = "ReticulumNode",
    permissions = {
        @Permission(
            strings = { Manifest.permission.BLUETOOTH_SCAN, Manifest.permission.BLUETOOTH_CONNECT },
            alias = ReticulumNodePlugin.RNODE_BLUETOOTH_ALIAS
        )
    }
)
public class ReticulumNodePlugin extends ReticulumNodeTransportPluginApi {

    @PluginMethod
    public void refreshPlugins(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to refresh plugins.",
            "Native plugin catalog JSON parse failed.",
            ReticulumNodeService::refreshPluginsJson
        );
    }

    @PluginMethod
    public void listPlugins(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to list plugins.",
            "Native plugin catalog JSON parse failed.",
            ReticulumNodeService::listPluginsJson
        );
    }

    @PluginMethod
    public void approvePluginPublisher(PluginCall call) {
        final String pluginId = call.getString("pluginId", "");
        if (pluginId.isEmpty()) {
            call.reject("pluginId is required.");
            return;
        }
        final JSObject payload = new JSObject();
        payload.put("pluginId", pluginId);
        payload.put("displayName", call.getString("displayName"));
        runIntServiceCall(
            call,
            "Failed to approve plugin publisher.",
            service -> service.approvePluginPublisherJson(payload.toString())
        );
    }

    @PluginMethod
    public void revokePluginPublisher(PluginCall call) {
        final String fingerprint = call.getString("fingerprint", "");
        if (fingerprint.isEmpty()) {
            call.reject("fingerprint is required.");
            return;
        }
        final JSObject payload = new JSObject();
        payload.put("fingerprint", fingerprint);
        runIntServiceCall(
            call,
            "Failed to revoke plugin publisher.",
            service -> service.revokePluginPublisherJson(payload.toString())
        );
    }

    @PluginMethod
    public void setPluginEnabled(PluginCall call) {
        final String pluginId = call.getString("pluginId", "");
        final Boolean enabled = call.getBoolean("enabled");
        if (pluginId.isEmpty() || enabled == null) {
            call.reject("pluginId and enabled are required.");
            return;
        }
        final JSObject payload = new JSObject();
        payload.put("pluginId", pluginId);
        payload.put("enabled", enabled);
        runIntServiceCall(
            call,
            "Failed to update plugin enablement.",
            service -> service.setPluginEnabledJson(payload.toString())
        );
    }

    @PluginMethod
    public void grantPluginCapabilities(PluginCall call) {
        final String pluginId = call.getString("pluginId", "");
        final JSObject capabilities = call.getObject("capabilities");
        if (pluginId.isEmpty() || capabilities == null) {
            call.reject("pluginId and capabilities are required.");
            return;
        }
        final JSObject payload = new JSObject();
        payload.put("pluginId", pluginId);
        payload.put("capabilities", capabilities);
        runIntServiceCall(
            call,
            "Failed to update plugin capabilities.",
            service -> service.grantPluginCapabilitiesJson(payload.toString())
        );
    }

    @PluginMethod
    public void listPluginSensors(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to list plugin sensors.",
            "Native plugin sensor JSON parse failed.",
            ReticulumNodeService::listPluginSensorsJson
        );
    }

    @PluginMethod
    public void openPluginConfiguration(PluginCall call) {
        final String pluginId = call.getString("pluginId", "");
        if (pluginId.isEmpty()) {
            call.reject("pluginId is required.");
            return;
        }
        executeBridgeTask(() -> {
            try {
                final ReticulumNodeService service = awaitService();
                final Intent intent = service.pluginConfigurationIntent(pluginId);
                if (intent == null) {
                    call.reject("Plugin configuration is unavailable.");
                    return;
                }
                getContext().startActivity(intent);
                call.resolve();
            } catch (Exception error) {
                call.reject("Failed to open plugin configuration.", error);
            }
        });
    }

    @PluginMethod
    public void setActivePropagationNode(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("destinationHex", call.getString("destinationHex"));
        runIntServiceCall(
            call,
            "Failed to set active propagation node.",
            service -> service.setActivePropagationNodeJson(payload.toString())
        );
    }

    @PluginMethod
    public void requestLxmfSync(PluginCall call) {
        final JSObject payload = new JSObject();
        final Integer limit = call.getInt("limit");
        if (limit != null) {
            payload.put("limit", limit);
        } else {
            payload.put("limit", null);
        }
        runIntServiceCall(
            call,
            "Failed to request LXMF sync.",
            service -> service.requestLxmfSyncJson(payload.toString())
        );
    }

    @PluginMethod
    public void listAnnounces(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to list announces.",
            "Native announce list JSON parse failed.",
            ReticulumNodeService::listAnnouncesJson
        );
    }

    @PluginMethod
    public void listPeers(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to list peers.",
            "Native peer list JSON parse failed.",
            ReticulumNodeService::listPeersJson
        );
    }

    @PluginMethod
    public void listConversations(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to list conversations.",
            "Native conversation list JSON parse failed.",
            ReticulumNodeService::listConversationsJson
        );
    }

    @PluginMethod
    public void listMessages(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("conversationId", call.getString("conversationId"));
        runStringServiceCall(
            call,
            "Failed to list messages.",
            "Native message list JSON parse failed.",
            service -> service.listMessagesJson(payload.toString())
        );
    }

    @PluginMethod
    public void deleteConversation(PluginCall call) {
        final String conversationId = call.getString("conversationId");
        if (conversationId == null || conversationId.trim().isEmpty()) {
            call.reject("conversationId is required.");
            return;
        }
        final JSObject payload = new JSObject();
        payload.put("conversationId", conversationId);
        runIntServiceCall(
            call,
            "Failed to delete conversation.",
            service -> service.deleteConversationJson(payload.toString())
        );
    }

    @PluginMethod
    public void getLxmfSyncStatus(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to get LXMF sync status.",
            "Native sync status JSON parse failed.",
            ReticulumNodeService::getLxmfSyncStatusJson
        );
    }

    @PluginMethod
    public void listTelemetryDestinations(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to list telemetry destinations.",
            "Native telemetry destinations JSON parse failed.",
            ReticulumNodeService::listTelemetryDestinationsJson
        );
    }

    @PluginMethod
    public void legacyImportCompleted(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to read legacy import state.",
            "Native legacy import JSON parse failed.",
            ReticulumNodeService::legacyImportCompletedJson
        );
    }

    @PluginMethod
    public void importLegacyState(PluginCall call) {
        final JSObject payload = call.getObject("payload", new JSObject());
        runIntServiceCall(
            call,
            "Failed to import legacy state.",
            service -> service.importLegacyStateJson(payload.toString())
        );
    }

    @PluginMethod
    public void getAppSettings(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to get app settings.",
            "Native app settings JSON parse failed.",
            ReticulumNodeService::getAppSettingsJson
        );
    }

    @PluginMethod
    public void setAppSettings(PluginCall call) {
        final JSObject payload = call.getObject("settings", new JSObject());
        runIntServiceCall(
            call,
            "Failed to save app settings.",
            service -> service.setAppSettingsJson(payload.toString())
        );
    }

    @PluginMethod
    public void getWatchStatusServerSettings(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to get watch status server settings.",
            "Native watch status server settings JSON parse failed.",
            ReticulumNodeService::getWatchStatusServerSettingsJson
        );
    }

    @PluginMethod
    public void setWatchStatusServerSettings(PluginCall call) {
        final JSObject payload = new JSObject();
        final Boolean enabled = call.getBoolean("enabled");
        final Integer port = call.getInt("port");
        if (enabled != null) {
            payload.put("enabled", enabled);
        }
        if (port != null) {
            payload.put("port", port);
        }
        runIntServiceCall(
            call,
            "Failed to save watch status server settings.",
            service -> service.setWatchStatusServerSettingsJson(payload.toString())
        );
    }

    @PluginMethod
    public void getWatchStatusServerState(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to get watch status server state.",
            "Native watch status server state JSON parse failed.",
            ReticulumNodeService::getWatchStatusServerStateJson
        );
    }

    @PluginMethod
    public void getSavedPeers(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to get saved peers.",
            "Native saved peers JSON parse failed.",
            ReticulumNodeService::getSavedPeersJson
        );
    }

    @PluginMethod
    public void setSavedPeers(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("savedPeers", call.getArray("savedPeers"));
        runIntServiceCall(
            call,
            "Failed to save peers.",
            service -> service.setSavedPeersJson(payload.toString())
        );
    }

    @PluginMethod
    public void getOperationalSummary(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to get operational summary.",
            "Native operational summary JSON parse failed.",
            ReticulumNodeService::getOperationalSummaryJson
        );
    }

    @PluginMethod
    public void getChecklists(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("search", call.getString("search"));
        payload.put("sortBy", call.getString("sortBy"));
        runStringServiceCall(
            call,
            "Failed to get checklists.",
            "Native checklist list JSON parse failed.",
            service -> service.getChecklistsJson(payload.toString())
        );
    }

    @PluginMethod
    public void getChecklist(PluginCall call) {
        final String checklistUid = call.getString("checklistUid");
        if (checklistUid == null || checklistUid.trim().isEmpty()) {
            call.reject("checklistUid is required.");
            return;
        }
        final JSObject payload = new JSObject();
        payload.put("checklistUid", checklistUid);
        runStringServiceCall(
            call,
            "Failed to get checklist.",
            "Native checklist detail JSON parse failed.",
            service -> service.getChecklistJson(payload.toString())
        );
    }

    @PluginMethod
    public void getChecklistTemplates(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("search", call.getString("search"));
        payload.put("sortBy", call.getString("sortBy"));
        runStringServiceCall(
            call,
            "Failed to get checklist templates.",
            "Native checklist template JSON parse failed.",
            service -> service.getChecklistTemplatesJson(payload.toString())
        );
    }

    @PluginMethod
    public void importChecklistTemplateCsv(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("templateUid", call.getString("templateUid"));
        payload.put("name", call.getString("name", ""));
        payload.put("description", call.getString("description"));
        payload.put("csvText", call.getString("csvText", ""));
        payload.put("sourceFilename", call.getString("sourceFilename"));
        runStringServiceCall(
            call,
            "Failed to import checklist template CSV.",
            "Native checklist template import JSON parse failed.",
            service -> service.importChecklistTemplateCsvJson(payload.toString())
        );
    }

    @PluginMethod
    public void createChecklistFromTemplate(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("checklistUid", call.getString("checklistUid"));
        payload.put("missionUid", call.getString("missionUid"));
        payload.put("templateUid", call.getString("templateUid", ""));
        payload.put("name", call.getString("name", ""));
        payload.put("description", call.getString("description", ""));
        payload.put("startTime", call.getString("startTime", ""));
        payload.put("createdByTeamMemberRnsIdentity", call.getString("createdByTeamMemberRnsIdentity"));
        runIntServiceCall(
            call,
            "Failed to create checklist from template.",
            service -> service.createChecklistFromTemplateJson(payload.toString())
        );
    }

    @PluginMethod
    public void createOnlineChecklist(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("checklistUid", call.getString("checklistUid"));
        payload.put("missionUid", call.getString("missionUid"));
        payload.put("templateUid", call.getString("templateUid", ""));
        payload.put("name", call.getString("name", ""));
        payload.put("description", call.getString("description", ""));
        payload.put("startTime", call.getString("startTime", ""));
        payload.put("createdByTeamMemberRnsIdentity", call.getString("createdByTeamMemberRnsIdentity"));
        runIntServiceCall(
            call,
            "Failed to create online checklist.",
            service -> service.createOnlineChecklistJson(payload.toString())
        );
    }

    @PluginMethod
    public void updateChecklist(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("checklistUid", call.getString("checklistUid"));
        payload.put("patch", call.getObject("patch", new JSObject()));
        runIntServiceCall(
            call,
            "Failed to update checklist.",
            service -> service.updateChecklistJson(payload.toString())
        );
    }

    @PluginMethod
    public void deleteChecklist(PluginCall call) {
        final String checklistUid = call.getString("checklistUid");
        if (checklistUid == null || checklistUid.trim().isEmpty()) {
            call.reject("checklistUid is required.");
            return;
        }
        final JSObject payload = new JSObject();
        payload.put("checklistUid", checklistUid);
        payload.put("deleteRemote", Boolean.TRUE.equals(call.getBoolean("deleteRemote")));
        runIntServiceCall(
            call,
            "Failed to delete checklist.",
            service -> service.deleteChecklistJson(payload.toString())
        );
    }

    @PluginMethod
    public void joinChecklist(PluginCall call) {
        final String checklistUid = call.getString("checklistUid");
        if (checklistUid == null || checklistUid.trim().isEmpty()) {
            call.reject("checklistUid is required.");
            return;
        }
        final JSObject payload = new JSObject();
        payload.put("checklistUid", checklistUid);
        runIntServiceCall(
            call,
            "Failed to join checklist.",
            service -> service.joinChecklistJson(payload.toString())
        );
    }

    @PluginMethod
    public void uploadChecklist(PluginCall call) {
        final String checklistUid = call.getString("checklistUid");
        if (checklistUid == null || checklistUid.trim().isEmpty()) {
            call.reject("checklistUid is required.");
            return;
        }
        final JSObject payload = new JSObject();
        payload.put("checklistUid", checklistUid);
        runIntServiceCall(
            call,
            "Failed to upload checklist.",
            service -> service.uploadChecklistJson(payload.toString())
        );
    }

    @PluginMethod
    public void setChecklistTaskStatus(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("checklistUid", call.getString("checklistUid"));
        payload.put("taskUid", call.getString("taskUid"));
        payload.put("userStatus", call.getString("userStatus"));
        payload.put("changedByTeamMemberRnsIdentity", call.getString("changedByTeamMemberRnsIdentity"));
        runIntServiceCall(
            call,
            "Failed to set checklist task status.",
            service -> service.setChecklistTaskStatusJson(payload.toString())
        );
    }

    @PluginMethod
    public void addChecklistTaskRow(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("checklistUid", call.getString("checklistUid"));
        payload.put("taskUid", call.getString("taskUid"));
        payload.put("number", call.getInt("number"));
        payload.put("dueRelativeMinutes", call.getInt("dueRelativeMinutes"));
        payload.put("legacyValue", call.getString("legacyValue"));
        runIntServiceCall(
            call,
            "Failed to add checklist task row.",
            service -> service.addChecklistTaskRowJson(payload.toString())
        );
    }

    @PluginMethod
    public void deleteChecklistTaskRow(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("checklistUid", call.getString("checklistUid"));
        payload.put("taskUid", call.getString("taskUid"));
        runIntServiceCall(
            call,
            "Failed to delete checklist task row.",
            service -> service.deleteChecklistTaskRowJson(payload.toString())
        );
    }

    @PluginMethod
    public void setChecklistTaskRowStyle(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("checklistUid", call.getString("checklistUid"));
        payload.put("taskUid", call.getString("taskUid"));
        payload.put("rowBackgroundColor", call.getString("rowBackgroundColor"));
        final Boolean lineBreakEnabled = call.getBoolean("lineBreakEnabled");
        if (lineBreakEnabled != null) {
            payload.put("lineBreakEnabled", lineBreakEnabled);
        }
        runIntServiceCall(
            call,
            "Failed to set checklist task row style.",
            service -> service.setChecklistTaskRowStyleJson(payload.toString())
        );
    }

    @PluginMethod
    public void setChecklistTaskCell(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("checklistUid", call.getString("checklistUid"));
        payload.put("taskUid", call.getString("taskUid"));
        payload.put("columnUid", call.getString("columnUid"));
        payload.put("value", call.getString("value"));
        payload.put("updatedByTeamMemberRnsIdentity", call.getString("updatedByTeamMemberRnsIdentity"));
        runIntServiceCall(
            call,
            "Failed to set checklist task cell.",
            service -> service.setChecklistTaskCellJson(payload.toString())
        );
    }

    @PluginMethod
    public void getEams(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to get EAMs.",
            "Native EAM JSON parse failed.",
            ReticulumNodeService::getEamsJson
        );
    }

    @PluginMethod
    public void upsertEam(PluginCall call) {
        final JSObject payload = call.getObject("eam", new JSObject());
        runIntServiceCall(
            call,
            "Failed to save EAM.",
            service -> service.upsertEamJson(payload.toString())
        );
    }

    @PluginMethod
    public void deleteEam(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("callsign", call.getString("callsign"));
        final Long deletedAtMs = call.getLong("deletedAtMs");
        if (deletedAtMs != null) {
            payload.put("deletedAtMs", deletedAtMs);
        }
        runIntServiceCall(
            call,
            "Failed to delete EAM.",
            service -> service.deleteEamJson(payload.toString())
        );
    }

    @PluginMethod
    public void deleteLocalEam(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("callsign", call.getString("callsign"));
        final Long deletedAtMs = call.getLong("deletedAtMs");
        if (deletedAtMs != null) {
            payload.put("deletedAtMs", deletedAtMs);
        }
        runIntServiceCall(
            call,
            "Failed to delete local EAM.",
            service -> service.deleteLocalEamJson(payload.toString())
        );
    }

    @PluginMethod
    public void getEamTeamSummary(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("teamUid", call.getString("teamUid"));
        runStringServiceCall(
            call,
            "Failed to get EAM team summary.",
            "Native EAM team summary JSON parse failed.",
            service -> service.getEamTeamSummaryJson(payload.toString())
        );
    }

    @PluginMethod
    public void getEamReadinessSummary(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to get EAM readiness summary.",
            "Native EAM readiness summary JSON parse failed.",
            ReticulumNodeService::getEamReadinessSummaryJson
        );
    }

    @PluginMethod
    public void getEvents(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to get events.",
            "Native events JSON parse failed.",
            ReticulumNodeService::getEventsJson
        );
    }

    @PluginMethod
    public void upsertEvent(PluginCall call) {
        final JSObject payload = call.getObject("event", new JSObject());
        runIntServiceCall(
            call,
            "Failed to save event.",
            service -> service.upsertEventJson(payload.toString())
        );
    }

    @PluginMethod
    public void deleteEvent(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("uid", call.getString("uid"));
        final Long deletedAtMs = call.getLong("deletedAtMs");
        if (deletedAtMs != null) {
            payload.put("deletedAtMs", deletedAtMs);
        }
        runIntServiceCall(
            call,
            "Failed to delete event.",
            service -> service.deleteEventJson(payload.toString())
        );
    }

    @PluginMethod
    public void getTelemetryPositions(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to get telemetry positions.",
            "Native telemetry JSON parse failed.",
            ReticulumNodeService::getTelemetryPositionsJson
        );
    }

    @PluginMethod
    public void recordLocalTelemetryFix(PluginCall call) {
        final JSObject payload = call.getObject("position", new JSObject());
        runIntServiceCall(
            call,
            "Failed to record local telemetry.",
            service -> service.recordLocalTelemetryFixJson(payload.toString())
        );
    }

    @PluginMethod
    public void deleteLocalTelemetry(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("callsign", call.getString("callsign"));
        runIntServiceCall(
            call,
            "Failed to delete local telemetry.",
            service -> service.deleteLocalTelemetryJson(payload.toString())
        );
    }

    @PluginMethod
    public void getSosSettings(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to get SOS settings.",
            "Native SOS settings JSON parse failed.",
            ReticulumNodeService::getSosSettingsJson
        );
    }

    @PluginMethod
    public void setSosSettings(PluginCall call) {
        final JSObject payload = call.getObject("settings", new JSObject());
        runIntServiceCall(
            call,
            "Failed to save SOS settings.",
            service -> service.setSosSettingsJson(payload.toString())
        );
    }

    @PluginMethod
    public void setSosPin(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("pin", call.getString("pin"));
        runIntServiceCall(
            call,
            "Failed to set SOS PIN.",
            service -> service.setSosPinJson(payload.toString())
        );
    }

    @PluginMethod
    public void getSosStatus(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to get SOS status.",
            "Native SOS status JSON parse failed.",
            ReticulumNodeService::getSosStatusJson
        );
    }

    @PluginMethod
    public void triggerSos(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("source", call.getString("source", "Manual"));
        runStringServiceCall(
            call,
            "Failed to trigger SOS.",
            "Native SOS trigger JSON parse failed.",
            service -> service.triggerSosJson(payload.toString())
        );
    }

    @PluginMethod
    public void deactivateSos(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("pin", call.getString("pin"));
        runStringServiceCall(
            call,
            "Failed to deactivate SOS.",
            "Native SOS deactivate JSON parse failed.",
            service -> service.deactivateSosJson(payload.toString())
        );
    }

    @PluginMethod
    public void submitSosTelemetry(PluginCall call) {
        final JSObject payload = call.getObject("telemetry", new JSObject());
        runIntServiceCall(
            call,
            "Failed to submit SOS telemetry.",
            service -> service.submitSosTelemetryJson(payload.toString())
        );
    }

    @PluginMethod
    public void listSosAlerts(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to list SOS alerts.",
            "Native SOS alerts JSON parse failed.",
            ReticulumNodeService::listSosAlertsJson
        );
    }

    @PluginMethod
    public void listSosLocations(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to list SOS locations.",
            "Native SOS locations JSON parse failed.",
            ReticulumNodeService::listSosLocationsJson
        );
    }

    @PluginMethod
    public void listSosAudio(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to list SOS audio.",
            "Native SOS audio JSON parse failed.",
            ReticulumNodeService::listSosAudioJson
        );
    }

    @PluginMethod
    public void recordSosAudio(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("audioId", call.getString("audioId"));
        payload.put("incidentId", call.getString("incidentId"));
        payload.put("sourceHex", call.getString("sourceHex"));
        payload.put("path", call.getString("path"));
        payload.put("mimeType", call.getString("mimeType"));
        payload.put("durationSeconds", call.getInt("durationSeconds"));
        payload.put("createdAtMs", call.getLong("createdAtMs"));
        runIntServiceCall(
            call,
            "Failed to record SOS audio.",
            service -> service.recordSosAudioJson(payload.toString())
        );
    }

    @PluginMethod
    public void removeAllListeners(PluginCall call) {
        call.resolve();
    }

}
