package network.reticulum.emergency;

import com.getcapacitor.JSObject;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;

public abstract class ReticulumNodeChecklistPluginApi extends ReticulumNodeAppDataPluginApi {

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
}
