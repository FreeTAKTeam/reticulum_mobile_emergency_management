package network.reticulum.emergency.plugin.api;

import network.reticulum.emergency.plugin.api.IRemPluginConfigurationCallback;
import network.reticulum.emergency.plugin.api.IRemPluginHost;

interface IRemPluginService {
    String getDescriptorJson();
    void start(in IRemPluginHost host, String sessionJson);
    void stop(String reason);
    oneway void onHostEvent(String eventJson);
    oneway void onHostResponse(String responseJson);
    oneway void handleConfigurationRequest(
        String requestJson,
        in IRemPluginConfigurationCallback callback
    );
}
