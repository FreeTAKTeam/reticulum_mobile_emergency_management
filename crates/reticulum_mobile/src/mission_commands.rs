#[cfg(test)]
pub(crate) use r3akt_mission_wire::command_code;
pub(crate) use r3akt_mission_wire::{
    canonical_command_type, checklist_arg_code, checklist_arg_wire_key, command_wire_value,
};

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
