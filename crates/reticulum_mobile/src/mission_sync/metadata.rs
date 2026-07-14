#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct MissionSyncMetadata {
    pub(crate) command_present: bool,
    pub(crate) result_present: bool,
    pub(crate) event_present: bool,
    pub(crate) correlation_id: Option<String>,
    pub(crate) command_id: Option<String>,
    pub(crate) command_type: Option<String>,
    pub(crate) result_status: Option<String>,
    pub(crate) event_type: Option<String>,
    pub(crate) event_uid: Option<String>,
    pub(crate) eam_uid: Option<String>,
    pub(crate) team_member_uid: Option<String>,
    pub(crate) team_uid: Option<String>,
    pub(crate) mission_uid: Option<String>,
    pub(crate) checklist_uid: Option<String>,
    pub(crate) task_uid: Option<String>,
    pub(crate) column_uid: Option<String>,
}

impl MissionSyncMetadata {
    pub(crate) fn tracking_key(&self) -> Option<&str> {
        self.command_id
            .as_deref()
            .or(self.correlation_id.as_deref())
    }

    pub(crate) fn primary_kind(&self) -> &'static str {
        if self.command_present {
            "command"
        } else if self.result_present {
            "result"
        } else if self.event_present {
            "event"
        } else {
            "message"
        }
    }

    pub(crate) fn primary_name(&self) -> Option<&str> {
        self.command_type
            .as_deref()
            .or(self.result_status.as_deref())
            .or(self.event_type.as_deref())
    }

    pub(crate) fn ack_detail(&self) -> Option<&str> {
        self.result_status
            .as_deref()
            .or(self.event_type.as_deref())
            .or(self.command_type.as_deref())
    }

    pub(crate) fn is_mission_related(&self) -> bool {
        self.command_present
            || self.result_present
            || self.event_present
            || self.command_id.is_some()
            || self.correlation_id.is_some()
            || self.command_type.is_some()
            || self.result_status.is_some()
            || self.event_type.is_some()
            || self.event_uid.is_some()
            || self.eam_uid.is_some()
            || self.team_member_uid.is_some()
            || self.team_uid.is_some()
            || self.mission_uid.is_some()
            || self.checklist_uid.is_some()
            || self.task_uid.is_some()
            || self.column_uid.is_some()
    }

    pub(crate) fn is_event_related(&self) -> bool {
        self.is_mission_related()
    }
}
