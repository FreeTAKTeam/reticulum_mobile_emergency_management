pub(crate) fn command_code(command_type: &str) -> Option<&'static str> {
    match command_type {
        "mission.registry.log_entry.upsert" => Some("E1"),
        "mission.registry.log_entry.upserted" => Some("E2"),
        "mission.registry.eam.upsert" => Some("M1"),
        "mission.registry.eam.delete" => Some("M2"),
        "mission.registry.eam.upserted" => Some("M3"),
        "mission.registry.eam.list" => Some("M4"),
        "mission.registry.eam.get" => Some("M5"),
        "mission.registry.eam.latest" => Some("M6"),
        "mission.registry.eam.team.summary" => Some("M7"),
        "mission.registry.eam.listed" => Some("M8"),
        "mission.registry.eam.retrieved" => Some("M9"),
        "mission.registry.eam.latest_retrieved" => Some("MA"),
        "mission.registry.eam.deleted" => Some("MB"),
        "mission.registry.eam.team_summary.retrieved" => Some("MC"),
        "mission.registry.team.list" => Some("H1"),
        "mission.registry.team.upsert" => Some("H2"),
        "mission.registry.team_member.list" => Some("H3"),
        "mission.registry.team_member.upsert" => Some("H4"),
        "mission.registry.team_member.client.link" => Some("H5"),
        "mission.registry.telemetry.upsert" => Some("T1"),
        "sos.status" => Some("S1"),
        "checklist.create.online" => Some("C1"),
        "checklist.upload" => Some("C2"),
        "checklist.update" => Some("C3"),
        "checklist.delete" => Some("C4"),
        "checklist.join" => Some("C5"),
        "checklist.task.status.set" => Some("C6"),
        "checklist.task.row.add" => Some("C7"),
        "checklist.task.row.delete" => Some("C8"),
        "checklist.task.row.style.set" => Some("C9"),
        "checklist.task.cell.set" => Some("CA"),
        _ => None,
    }
}

pub(crate) fn canonical_command_type(command_type_or_code: &str) -> &str {
    match command_type_or_code {
        "E1" => "mission.registry.log_entry.upsert",
        "E2" => "mission.registry.log_entry.upserted",
        "M1" => "mission.registry.eam.upsert",
        "M2" => "mission.registry.eam.delete",
        "M3" => "mission.registry.eam.upserted",
        "M4" => "mission.registry.eam.list",
        "M5" => "mission.registry.eam.get",
        "M6" => "mission.registry.eam.latest",
        "M7" => "mission.registry.eam.team.summary",
        "M8" => "mission.registry.eam.listed",
        "M9" => "mission.registry.eam.retrieved",
        "MA" => "mission.registry.eam.latest_retrieved",
        "MB" => "mission.registry.eam.deleted",
        "MC" => "mission.registry.eam.team_summary.retrieved",
        "H1" => "mission.registry.team.list",
        "H2" => "mission.registry.team.upsert",
        "H3" => "mission.registry.team_member.list",
        "H4" => "mission.registry.team_member.upsert",
        "H5" => "mission.registry.team_member.client.link",
        "T1" => "mission.registry.telemetry.upsert",
        "S1" => "sos.status",
        "C1" => "checklist.create.online",
        "C2" => "checklist.upload",
        "C3" => "checklist.update",
        "C4" => "checklist.delete",
        "C5" => "checklist.join",
        "C6" => "checklist.task.status.set",
        "C7" => "checklist.task.row.add",
        "C8" => "checklist.task.row.delete",
        "C9" => "checklist.task.row.style.set",
        "CA" => "checklist.task.cell.set",
        legacy => legacy,
    }
}

pub(crate) fn command_wire_value(command_type: &str) -> &str {
    command_code(command_type).unwrap_or(command_type)
}

pub(crate) fn checklist_arg_code(key: &str) -> Option<&'static str> {
    match key {
        "checklist_uid" | "checklistUid" => Some("cl"),
        "mission_uid" | "missionUid" => Some("m"),
        "template_uid" | "templateUid" => Some("tp"),
        "name" => Some("n"),
        "description" => Some("d"),
        "start_time" | "startTime" => Some("st"),
        "columns" => Some("cols"),
        "tasks" => Some("tasks"),
        "participant_rns_identities" | "participantRnsIdentities" => Some("p"),
        "created_at" | "createdAt" => Some("ca"),
        "created_by_team_member_rns_identity" | "createdByTeamMemberRnsIdentity" => Some("cr"),
        "created_by_team_member_display_name" | "createdByTeamMemberDisplayName" => Some("cdn"),
        "total_tasks" | "totalTasks" => Some("tt"),
        "uploaded_at" | "uploadedAt" => Some("ua"),
        "patch" => Some("pa"),
        "task_uid" | "taskUid" => Some("tsk"),
        "number" => Some("no"),
        "due_relative_minutes" | "dueRelativeMinutes" => Some("dr"),
        "due_dtg" | "dueDtg" => Some("dd"),
        "notes" => Some("nt"),
        "legacy_value" | "legacyValue" => Some("lv"),
        "changed_by_team_member_rns_identity" | "changedByTeamMemberRnsIdentity" => Some("cb"),
        "user_status" | "userStatus" => Some("us"),
        "completed" => Some("x"),
        "row_background_color" | "rowBackgroundColor" => Some("bg"),
        "line_break_enabled" | "lineBreakEnabled" => Some("lb"),
        "column_uid" | "columnUid" => Some("col"),
        "column_name" | "columnName" => Some("cn"),
        "display_order" | "displayOrder" => Some("ord"),
        "column_type" | "columnType" => Some("ct"),
        "column_editable" | "columnEditable" => Some("ce"),
        "text_color" | "textColor" => Some("tc"),
        "is_removable" | "isRemovable" => Some("rm"),
        "system_key" | "systemKey" => Some("sk"),
        "value" => Some("v"),
        "updated_by_team_member_rns_identity" | "updatedByTeamMemberRnsIdentity" => Some("ub"),
        "task" => Some("tr"),
        "snapshot" => Some("sn"),
        "snapshot_json" | "snapshotJson" => Some("sj"),
        _ => None,
    }
}

pub(crate) fn checklist_arg_wire_key(key: &str) -> &str {
    checklist_arg_code(key).unwrap_or(key)
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_command_type, checklist_arg_code, checklist_arg_wire_key, command_code,
        command_wire_value,
    };

    #[test]
    fn command_codes_are_alphanumeric_and_round_trip() {
        for command in [
            "mission.registry.log_entry.upsert",
            "mission.registry.log_entry.upserted",
            "mission.registry.eam.upsert",
            "mission.registry.eam.delete",
            "mission.registry.eam.upserted",
            "mission.registry.eam.list",
            "mission.registry.eam.get",
            "mission.registry.eam.latest",
            "mission.registry.eam.team.summary",
            "mission.registry.eam.listed",
            "mission.registry.eam.retrieved",
            "mission.registry.eam.latest_retrieved",
            "mission.registry.eam.deleted",
            "mission.registry.eam.team_summary.retrieved",
            "mission.registry.team.list",
            "mission.registry.team.upsert",
            "mission.registry.team_member.list",
            "mission.registry.team_member.upsert",
            "mission.registry.team_member.client.link",
            "mission.registry.telemetry.upsert",
            "sos.status",
            "checklist.create.online",
            "checklist.upload",
            "checklist.update",
            "checklist.delete",
            "checklist.join",
            "checklist.task.status.set",
            "checklist.task.row.add",
            "checklist.task.row.delete",
            "checklist.task.row.style.set",
            "checklist.task.cell.set",
        ] {
            let code = command_code(command).expect("known command code");
            assert!(code.chars().all(|ch| ch.is_ascii_alphanumeric()));
            assert_eq!(canonical_command_type(code), command);
            assert_eq!(command_wire_value(command), code);
        }
    }

    #[test]
    fn checklist_arg_codes_are_short_and_stable() {
        for (expanded, code) in [
            ("checklist_uid", "cl"),
            ("mission_uid", "m"),
            ("template_uid", "tp"),
            ("task_uid", "tsk"),
            ("column_uid", "col"),
            ("column_name", "cn"),
            ("display_order", "ord"),
            ("column_type", "ct"),
            ("column_editable", "ce"),
            ("text_color", "tc"),
            ("is_removable", "rm"),
            ("system_key", "sk"),
            ("user_status", "us"),
            ("completed", "x"),
            ("updated_by_team_member_rns_identity", "ub"),
        ] {
            assert_eq!(checklist_arg_code(expanded), Some(code));
            assert_eq!(checklist_arg_wire_key(expanded), code);
        }
    }
}
