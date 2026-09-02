package network.reticulum.emergency;

import android.Manifest;

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
public class ReticulumNodePlugin extends ReticulumNodeChecklistPluginApi {

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
    public void getCommunityStatuses(PluginCall call) {
        runStringServiceCall(
            call,
            "Failed to get community statuses.",
            "Native community status JSON parse failed.",
            ReticulumNodeService::getCommunityStatusesJson
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
