import { asRecord } from "./records";

const COMMAND_TYPE_BY_CODE: Record<string, string> = {
  E1: "mission.registry.log_entry.upsert",
  E2: "mission.registry.log_entry.upserted",
  M1: "mission.registry.eam.upsert",
  M2: "mission.registry.eam.delete",
  M3: "mission.registry.eam.upserted",
  M4: "mission.registry.eam.list",
  M5: "mission.registry.eam.get",
  M6: "mission.registry.eam.latest",
  M7: "mission.registry.eam.team.summary",
  M8: "mission.registry.eam.listed",
  M9: "mission.registry.eam.retrieved",
  MA: "mission.registry.eam.latest_retrieved",
  MB: "mission.registry.eam.deleted",
  MC: "mission.registry.eam.team_summary.retrieved",
  H1: "mission.registry.team.list",
  H2: "mission.registry.team.upsert",
  H3: "mission.registry.team_member.list",
  H4: "mission.registry.team_member.upsert",
  H5: "mission.registry.team_member.client.link",
  T1: "mission.registry.telemetry.upsert",
  S1: "sos.status",
  C1: "checklist.create.online",
  C2: "checklist.upload",
  C3: "checklist.update",
  C4: "checklist.delete",
  C5: "checklist.join",
  C6: "checklist.task.status.set",
  C7: "checklist.task.row.add",
  C8: "checklist.task.row.delete",
  C9: "checklist.task.row.style.set",
  CA: "checklist.task.cell.set",
};

const COMMAND_CODE_BY_TYPE: Record<string, string> = Object.fromEntries(
  Object.entries(COMMAND_TYPE_BY_CODE).map(([code, commandType]) => [commandType, code]),
);

const CHECKLIST_ARG_CODE_BY_KEY: Record<string, string> = {
  checklist_uid: "cl", checklistUid: "cl", mission_uid: "m", missionUid: "m",
  template_uid: "tp", templateUid: "tp", name: "n", description: "d",
  start_time: "st", startTime: "st", columns: "cols", tasks: "tasks",
  participant_rns_identities: "p", participantRnsIdentities: "p",
  created_at: "ca", createdAt: "ca",
  created_by_team_member_rns_identity: "cr", createdByTeamMemberRnsIdentity: "cr",
  created_by_team_member_display_name: "cdn", createdByTeamMemberDisplayName: "cdn",
  total_tasks: "tt", totalTasks: "tt", uploaded_at: "ua", uploadedAt: "ua",
  patch: "pa", task_uid: "tsk", taskUid: "tsk", number: "no",
  due_relative_minutes: "dr", dueRelativeMinutes: "dr", due_dtg: "dd", dueDtg: "dd",
  notes: "nt", legacy_value: "lv", legacyValue: "lv",
  changed_by_team_member_rns_identity: "cb", changedByTeamMemberRnsIdentity: "cb",
  user_status: "us", userStatus: "us", row_background_color: "bg", rowBackgroundColor: "bg",
  line_break_enabled: "lb", lineBreakEnabled: "lb", column_uid: "col", columnUid: "col",
  column_name: "cn", columnName: "cn", display_order: "ord", displayOrder: "ord",
  column_type: "ct", columnType: "ct", column_editable: "ce", columnEditable: "ce",
  text_color: "tc", textColor: "tc", is_removable: "rm", isRemovable: "rm",
  system_key: "sk", systemKey: "sk", value: "v",
  updated_by_team_member_rns_identity: "ub", updatedByTeamMemberRnsIdentity: "ub",
  task: "tr", snapshot: "sn", snapshot_json: "sj", snapshotJson: "sj",
};

const CHECKLIST_ARG_KEY_BY_CODE: Record<string, string> = {
  cl: "checklist_uid", m: "mission_uid", tp: "template_uid", n: "name", d: "description",
  st: "start_time", cols: "columns", tasks: "tasks", p: "participant_rns_identities",
  ca: "created_at", cr: "created_by_team_member_rns_identity",
  cdn: "created_by_team_member_display_name", tt: "total_tasks", ua: "uploaded_at",
  pa: "patch", tsk: "task_uid", no: "number", dr: "due_relative_minutes", dd: "due_dtg",
  nt: "notes", lv: "legacy_value", cb: "changed_by_team_member_rns_identity",
  us: "user_status", bg: "row_background_color", lb: "line_break_enabled",
  col: "column_uid", cn: "column_name", ord: "display_order", ct: "column_type",
  ce: "column_editable", tc: "text_color", rm: "is_removable", sk: "system_key",
  v: "value", ub: "updated_by_team_member_rns_identity", tr: "task",
  sn: "snapshot", sj: "snapshot_json",
};

export function commandWireValue(commandType: string): string {
  return COMMAND_CODE_BY_TYPE[commandType] ?? commandType;
}

export function canonicalCommandType(value: string): string {
  return COMMAND_TYPE_BY_CODE[value] ?? value;
}

export function expandChecklistCommandArgs(
  args: Record<string, unknown>,
): Record<string, unknown> {
  const normalized: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(args)) {
    const expandedKey = CHECKLIST_ARG_KEY_BY_CODE[key] ?? key;
    normalized[expandedKey] = expandedKey === "patch" && asRecord(value)
      ? expandChecklistCommandArgs(asRecord(value) ?? {})
      : value;
  }
  return normalized;
}

export function compactMissionCommandArgs(
  commandType: string,
  args: Record<string, unknown>,
): Record<string, unknown> {
  if (!commandType.startsWith("checklist.")) return args;
  const compact: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(args)) {
    const wireKey = CHECKLIST_ARG_CODE_BY_KEY[key] ?? key;
    compact[wireKey] = key === "patch" && asRecord(value)
      ? compactMissionCommandArgs(commandType, asRecord(value) ?? {})
      : value;
  }
  return compact;
}
