package network.reticulum.emergency;

import android.app.Service;

public abstract class ReticulumBridgeServiceApi extends Service {
    public String getStatusJson() {
        return ReticulumBridge.getStatusJson();
    }

    public int connectPeer(String destinationHex) {
        return ReticulumBridge.connectPeer(destinationHex);
    }

    public int disconnectPeer(String destinationHex) {
        return ReticulumBridge.disconnectPeer(destinationHex);
    }

    public int announceNow() {
        return ReticulumBridge.announceNow();
    }

    public int requestPeerIdentity(String destinationHex) {
        return ReticulumBridge.requestPeerIdentity(destinationHex);
    }

    public int sendJson(String payloadJson) {
        return ReticulumBridge.sendJson(payloadJson);
    }

    public String sendLxmfJson(String payloadJson) {
        return ReticulumBridge.sendLxmfJson(payloadJson);
    }

    public int retryLxmfJson(String payloadJson) {
        return ReticulumBridge.retryLxmfJson(payloadJson);
    }

    public int cancelLxmfJson(String payloadJson) {
        return ReticulumBridge.cancelLxmfJson(payloadJson);
    }

    public int broadcastBase64(String bytesBase64) {
        return ReticulumBridge.broadcastBase64(bytesBase64);
    }

    public int setActivePropagationNodeJson(String payloadJson) {
        return ReticulumBridge.setActivePropagationNodeJson(payloadJson);
    }

    public int requestLxmfSyncJson(String payloadJson) {
        return ReticulumBridge.requestLxmfSyncJson(payloadJson);
    }

    public String listAnnouncesJson() {
        return ReticulumBridge.listAnnouncesJson();
    }

    public String listPeersJson() {
        return ReticulumBridge.listPeersJson();
    }

    public String listConversationsJson() {
        return ReticulumBridge.listConversationsJson();
    }

    public String listMessagesJson(String payloadJson) {
        return ReticulumBridge.listMessagesJson(payloadJson);
    }

    public int deleteConversationJson(String payloadJson) {
        return ReticulumBridge.deleteConversationJson(payloadJson);
    }

    public String getLxmfSyncStatusJson() {
        return ReticulumBridge.getLxmfSyncStatusJson();
    }

    public String listTelemetryDestinationsJson() {
        return ReticulumBridge.listTelemetryDestinationsJson();
    }

    public String legacyImportCompletedJson() {
        return ReticulumBridge.legacyImportCompletedJson();
    }

    public int importLegacyStateJson(String payloadJson) {
        return ReticulumBridge.importLegacyStateJson(payloadJson);
    }

    public String getAppSettingsJson() {
        return ReticulumBridge.getAppSettingsJson();
    }

    public int setAppSettingsJson(String payloadJson) {
        return ReticulumBridge.setAppSettingsJson(payloadJson);
    }

    public String getSavedPeersJson() {
        return ReticulumBridge.getSavedPeersJson();
    }

    public int setSavedPeersJson(String payloadJson) {
        return ReticulumBridge.setSavedPeersJson(payloadJson);
    }

    public String getOperationalSummaryJson() {
        return ReticulumBridge.getOperationalSummaryJson();
    }

    public String getChecklistsJson(String payloadJson) {
        return ReticulumBridge.getChecklistsJson(payloadJson);
    }

    public String getChecklistJson(String payloadJson) {
        return ReticulumBridge.getChecklistJson(payloadJson);
    }

    public String getChecklistTemplatesJson(String payloadJson) {
        return ReticulumBridge.getChecklistTemplatesJson(payloadJson);
    }

    public String importChecklistTemplateCsvJson(String payloadJson) {
        return ReticulumBridge.importChecklistTemplateCsvJson(payloadJson);
    }

    public int createChecklistFromTemplateJson(String payloadJson) {
        return ReticulumBridge.createChecklistFromTemplateJson(payloadJson);
    }

    public int createOnlineChecklistJson(String payloadJson) {
        return ReticulumBridge.createOnlineChecklistJson(payloadJson);
    }

    public int updateChecklistJson(String payloadJson) {
        return ReticulumBridge.updateChecklistJson(payloadJson);
    }

    public int deleteChecklistJson(String payloadJson) {
        return ReticulumBridge.deleteChecklistJson(payloadJson);
    }

    public int joinChecklistJson(String payloadJson) {
        return ReticulumBridge.joinChecklistJson(payloadJson);
    }

    public int uploadChecklistJson(String payloadJson) {
        return ReticulumBridge.uploadChecklistJson(payloadJson);
    }

    public int setChecklistTaskStatusJson(String payloadJson) {
        return ReticulumBridge.setChecklistTaskStatusJson(payloadJson);
    }

    public int addChecklistTaskRowJson(String payloadJson) {
        return ReticulumBridge.addChecklistTaskRowJson(payloadJson);
    }

    public int deleteChecklistTaskRowJson(String payloadJson) {
        return ReticulumBridge.deleteChecklistTaskRowJson(payloadJson);
    }

    public int setChecklistTaskRowStyleJson(String payloadJson) {
        return ReticulumBridge.setChecklistTaskRowStyleJson(payloadJson);
    }

    public int setChecklistTaskCellJson(String payloadJson) {
        return ReticulumBridge.setChecklistTaskCellJson(payloadJson);
    }

    public String getEamsJson() {
        return ReticulumBridge.getEamsJson();
    }

    public int upsertEamJson(String payloadJson) {
        return ReticulumBridge.upsertEamJson(payloadJson);
    }

    public int deleteEamJson(String payloadJson) {
        return ReticulumBridge.deleteEamJson(payloadJson);
    }

    public int deleteLocalEamJson(String payloadJson) {
        return ReticulumBridge.deleteLocalEamJson(payloadJson);
    }

    public String getEamTeamSummaryJson(String payloadJson) {
        return ReticulumBridge.getEamTeamSummaryJson(payloadJson);
    }

    public String getEamReadinessSummaryJson() {
        return ReticulumBridge.getEamReadinessSummaryJson();
    }

    public String getEventsJson() {
        return ReticulumBridge.getEventsJson();
    }

    public int upsertEventJson(String payloadJson) {
        return ReticulumBridge.upsertEventJson(payloadJson);
    }

    public int deleteEventJson(String payloadJson) {
        return ReticulumBridge.deleteEventJson(payloadJson);
    }

    public String getTelemetryPositionsJson() {
        return ReticulumBridge.getTelemetryPositionsJson();
    }

    public int recordLocalTelemetryFixJson(String payloadJson) {
        return ReticulumBridge.recordLocalTelemetryFixJson(payloadJson);
    }

    public int deleteLocalTelemetryJson(String payloadJson) {
        return ReticulumBridge.deleteLocalTelemetryJson(payloadJson);
    }

    public String getSosSettingsJson() {
        return ReticulumBridge.getSosSettingsJson();
    }

    public int setSosPinJson(String payloadJson) {
        return ReticulumBridge.setSosPinJson(payloadJson);
    }

    public String getSosStatusJson() {
        return ReticulumBridge.getSosStatusJson();
    }

    public String deactivateSosJson(String payloadJson) {
        return ReticulumBridge.deactivateSosJson(payloadJson);
    }

    public int submitSosTelemetryJson(String payloadJson) {
        return ReticulumBridge.submitSosTelemetryJson(payloadJson);
    }

    public String submitSosAccelerometerJson(String payloadJson) {
        return ReticulumBridge.submitSosAccelerometerJson(payloadJson);
    }

    public String submitSosScreenEventJson(String payloadJson) {
        return ReticulumBridge.submitSosScreenEventJson(payloadJson);
    }

    public String listSosAlertsJson() {
        return ReticulumBridge.listSosAlertsJson();
    }

    public String listSosLocationsJson() {
        return ReticulumBridge.listSosLocationsJson();
    }

    public String listSosAudioJson() {
        return ReticulumBridge.listSosAudioJson();
    }

    public int recordSosAudioJson(String payloadJson) {
        return ReticulumBridge.recordSosAudioJson(payloadJson);
    }

    public int setAnnounceCapabilities(String capabilityString) {
        return ReticulumBridge.setAnnounceCapabilities(capabilityString);
    }

    public int setLogLevel(String levelString) {
        return ReticulumBridge.setLogLevel(levelString);
    }

    public int refreshHubDirectory() {
        return ReticulumBridge.refreshHubDirectory();
    }

    public String getHubDirectorySnapshotJson() {
        return ReticulumBridge.getHubDirectorySnapshotJson();
    }

    public int setActiveTeamJson(String payloadJson) {
        return ReticulumBridge.setActiveTeamJson(payloadJson);
    }

    public String takeLastErrorJson() {
        return ReticulumBridge.takeLastErrorJson();
    }
}
