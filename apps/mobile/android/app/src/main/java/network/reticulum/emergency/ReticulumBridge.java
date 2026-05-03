package network.reticulum.emergency;

public final class ReticulumBridge {
    static {
        System.loadLibrary("reticulum_mobile");
    }

    private ReticulumBridge() {}

    public static native int initializeStorage(String storageDir);
    public static native int start(String configJson);
    public static native int stop();
    public static native int restart(String configJson);
    public static native String getStatusJson();
    public static native int connectPeer(String destinationHex);
    public static native int disconnectPeer(String destinationHex);
    public static native int announceNow();
    public static native int requestPeerIdentity(String destinationHex);
    public static native int sendJson(String payloadJson);
    public static native String sendLxmfJson(String payloadJson);
    public static native int sendPluginLxmfJson(String androidAbi, String payloadJson);
    public static native String decodePluginLxmfFieldsJson(String androidAbi, String payloadJson);
    public static native int retryLxmfJson(String payloadJson);
    public static native int cancelLxmfJson(String payloadJson);
    public static native int broadcastBase64(String bytesBase64);
    public static native int setActivePropagationNodeJson(String payloadJson);
    public static native int requestLxmfSyncJson(String payloadJson);
    public static native String listAnnouncesJson();
    public static native String listPeersJson();
    public static native String listConversationsJson();
    public static native String listMessagesJson(String payloadJson);
    public static native int deleteConversationJson(String payloadJson);
    public static native String getLxmfSyncStatusJson();
    public static native String listTelemetryDestinationsJson();
    public static native String legacyImportCompletedJson();
    public static native int importLegacyStateJson(String payloadJson);
    public static native String getAppSettingsJson();
    public static native int setAppSettingsJson(String payloadJson);
    public static native String getSavedPeersJson();
    public static native int setSavedPeersJson(String payloadJson);
    public static native String getOperationalSummaryJson();
    public static native String getChecklistsJson(String payloadJson);
    public static native String getChecklistJson(String payloadJson);
    public static native String getChecklistTemplatesJson(String payloadJson);
    public static native String importChecklistTemplateCsvJson(String payloadJson);
    public static native int createChecklistFromTemplateJson(String payloadJson);
    public static native int createOnlineChecklistJson(String payloadJson);
    public static native int updateChecklistJson(String payloadJson);
    public static native int deleteChecklistJson(String payloadJson);
    public static native int joinChecklistJson(String payloadJson);
    public static native int uploadChecklistJson(String payloadJson);
    public static native int setChecklistTaskStatusJson(String payloadJson);
    public static native int addChecklistTaskRowJson(String payloadJson);
    public static native int deleteChecklistTaskRowJson(String payloadJson);
    public static native int setChecklistTaskRowStyleJson(String payloadJson);
    public static native int setChecklistTaskCellJson(String payloadJson);
    public static native String getEamsJson();
    public static native int upsertEamJson(String payloadJson);
    public static native int deleteEamJson(String payloadJson);
    public static native String getEamTeamSummaryJson(String payloadJson);
    public static native String getEventsJson();
    public static native int upsertEventJson(String payloadJson);
    public static native int deleteEventJson(String payloadJson);
    public static native String getTelemetryPositionsJson();
    public static native int recordLocalTelemetryFixJson(String payloadJson);
    public static native int deleteLocalTelemetryJson(String payloadJson);
    public static native String getSosSettingsJson();
    public static native int setSosSettingsJson(String payloadJson);
    public static native int setSosPinJson(String payloadJson);
    public static native String getSosStatusJson();
    public static native String triggerSosJson(String payloadJson);
    public static native String deactivateSosJson(String payloadJson);
    public static native int submitSosTelemetryJson(String payloadJson);
    public static native String submitSosAccelerometerJson(String payloadJson);
    public static native String submitSosScreenEventJson(String payloadJson);
    public static native String listSosAlertsJson();
    public static native String listSosLocationsJson();
    public static native String listSosAudioJson();
    public static native int setAnnounceCapabilities(String capabilityString);
    public static native int setLogLevel(String levelString);
    public static native int refreshHubDirectory();
    public static native String getPluginsJson(String androidAbi);
    public static native String installPluginPackageJson(String androidAbi, String payloadJson);
    public static native int setPluginEnabledJson(String androidAbi, String payloadJson);
    public static native int grantPluginPermissionsJson(String androidAbi, String payloadJson);
    public static native String nextEventJson(int timeoutMs);
    public static native String takeLastErrorJson();
}
