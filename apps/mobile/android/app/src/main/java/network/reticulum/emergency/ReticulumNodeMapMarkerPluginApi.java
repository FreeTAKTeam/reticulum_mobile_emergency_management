package network.reticulum.emergency;

import com.getcapacitor.JSObject;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;

/**
 * Capacitor surface for shared map markers. Validation and persistence are owned by the
 * native runtime through r3akt-map-core; this layer only forwards JSON.
 */
public abstract class ReticulumNodeMapMarkerPluginApi extends ReticulumNodeAppDataPluginApi {

    @PluginMethod
    public void listMapMarkerSymbols(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to list map marker symbols.",
            "Native map marker symbol JSON parse failed.",
            ReticulumNodeService::listMapMarkerSymbolsJson
        );
    }

    @PluginMethod
    public void listMapMarkers(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to list map markers.",
            "Native map marker JSON parse failed.",
            ReticulumNodeService::listMapMarkersJson
        );
    }

    @PluginMethod
    public void createMapMarker(PluginCall call) {
        final JSObject request = call.getObject("request", new JSObject());
        runStringServiceCall(
            call,
            "Failed to create map marker.",
            "Native map marker JSON parse failed.",
            service -> service.createMapMarkerJson(request.toString())
        );
    }

    @PluginMethod
    public void renameMapMarker(PluginCall call) {
        final JSObject request = call.getObject("request", new JSObject());
        runStringServiceCall(
            call,
            "Failed to rename map marker.",
            "Native map marker JSON parse failed.",
            service -> service.renameMapMarkerJson(request.toString())
        );
    }

    @PluginMethod
    public void moveMapMarker(PluginCall call) {
        final JSObject request = call.getObject("request", new JSObject());
        runStringServiceCall(
            call,
            "Failed to move map marker.",
            "Native map marker JSON parse failed.",
            service -> service.moveMapMarkerJson(request.toString())
        );
    }

    @PluginMethod
    public void deleteMapMarker(PluginCall call) {
        final JSObject payload = new JSObject();
        payload.put("objectDestinationHash", call.getString("objectDestinationHash", ""));
        runStringServiceCall(
            call,
            "Failed to delete map marker.",
            "Native map marker JSON parse failed.",
            service -> service.deleteMapMarkerJson(payload.toString())
        );
    }
}
