package network.reticulum.emergency;

import android.content.Intent;

import com.getcapacitor.JSObject;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;

public abstract class ReticulumNodeAppDataPluginApi extends ReticulumNodeTransportPluginApi {

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
}
