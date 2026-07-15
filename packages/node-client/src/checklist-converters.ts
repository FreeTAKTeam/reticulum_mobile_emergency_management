import type { ChecklistCellRecord, ChecklistColumnRecord, ChecklistColumnType, ChecklistFeedPublicationRecord, ChecklistMode, ChecklistOriginType, ChecklistRecord, ChecklistSyncState, ChecklistTaskRecord, ChecklistTaskStatus, ChecklistTemplateRecord, ChecklistUserTaskStatus } from "./contracts";
import { toOptionalNumber } from "./converters";
import { hasValue, pluginRecord, toOptionalBoolean } from "./runtime-converters";

export function toChecklistColumnRecord(raw: Record<string, unknown>): ChecklistColumnRecord {
  return {
    columnUid: String(raw.columnUid ?? raw.column_uid ?? ""),
    columnName: String(raw.columnName ?? raw.column_name ?? ""),
    displayOrder: Number(raw.displayOrder ?? raw.display_order ?? 0),
    columnType: String(raw.columnType ?? raw.column_type ?? "SHORT_STRING") as ChecklistColumnType,
    columnEditable: Boolean(raw.columnEditable ?? raw.column_editable ?? true),
    backgroundColor: typeof raw.backgroundColor === "string"
      ? raw.backgroundColor
      : typeof raw.background_color === "string"
        ? raw.background_color
        : undefined,
    textColor: typeof raw.textColor === "string"
      ? raw.textColor
      : typeof raw.text_color === "string"
        ? raw.text_color
        : undefined,
    isRemovable: Boolean(raw.isRemovable ?? raw.is_removable ?? true),
    systemKey: typeof raw.systemKey === "string"
      ? raw.systemKey
      : typeof raw.system_key === "string"
        ? raw.system_key
        : undefined,
  };
}

export function toChecklistCellRecord(raw: Record<string, unknown>): ChecklistCellRecord {
  return {
    cellUid: String(raw.cellUid ?? raw.cell_uid ?? ""),
    taskUid: String(raw.taskUid ?? raw.task_uid ?? ""),
    columnUid: String(raw.columnUid ?? raw.column_uid ?? ""),
    value: typeof raw.value === "string" ? raw.value : undefined,
    updatedAt: typeof raw.updatedAt === "string"
      ? raw.updatedAt
      : typeof raw.updated_at === "string"
        ? raw.updated_at
        : undefined,
    updatedByTeamMemberRnsIdentity:
      typeof raw.updatedByTeamMemberRnsIdentity === "string"
        ? raw.updatedByTeamMemberRnsIdentity
        : typeof raw.updated_by_team_member_rns_identity === "string"
          ? raw.updated_by_team_member_rns_identity
          : undefined,
  };
}

export function toChecklistTaskRecord(raw: Record<string, unknown>): ChecklistTaskRecord {
  const cells = Array.isArray(raw.cells) ? raw.cells : [];
  return {
    taskUid: String(raw.taskUid ?? raw.task_uid ?? ""),
    number: Number(raw.number ?? 0),
    userStatus: String(raw.userStatus ?? raw.user_status ?? "PENDING") as ChecklistUserTaskStatus,
    taskStatus: String(raw.taskStatus ?? raw.task_status ?? "PENDING") as ChecklistTaskStatus,
    isLate: Boolean(raw.isLate ?? raw.is_late ?? false),
    updatedAt: typeof raw.updatedAt === "string"
      ? raw.updatedAt
      : typeof raw.updated_at === "string"
        ? raw.updated_at
        : undefined,
    deletedAt: typeof raw.deletedAt === "string"
      ? raw.deletedAt
      : typeof raw.deleted_at === "string"
        ? raw.deleted_at
        : undefined,
    customStatus: typeof raw.customStatus === "string"
      ? raw.customStatus
      : typeof raw.custom_status === "string"
        ? raw.custom_status
        : undefined,
    dueRelativeMinutes: toOptionalNumber(raw.dueRelativeMinutes ?? raw.due_relative_minutes),
    dueDtg: typeof raw.dueDtg === "string"
      ? raw.dueDtg
      : typeof raw.due_dtg === "string"
        ? raw.due_dtg
        : undefined,
    notes: typeof raw.notes === "string" ? raw.notes : undefined,
    rowBackgroundColor: typeof raw.rowBackgroundColor === "string"
      ? raw.rowBackgroundColor
      : typeof raw.row_background_color === "string"
        ? raw.row_background_color
        : undefined,
    lineBreakEnabled: toOptionalBoolean(raw.lineBreakEnabled ?? raw.line_break_enabled),
    legacyValue: typeof raw.legacyValue === "string"
      ? raw.legacyValue
      : typeof raw.legacy_value === "string"
        ? raw.legacy_value
        : undefined,
    completedAt: typeof raw.completedAt === "string"
      ? raw.completedAt
      : typeof raw.completed_at === "string"
        ? raw.completed_at
        : undefined,
    completedByTeamMemberRnsIdentity:
      typeof raw.completedByTeamMemberRnsIdentity === "string"
        ? raw.completedByTeamMemberRnsIdentity
        : typeof raw.completed_by_team_member_rns_identity === "string"
          ? raw.completed_by_team_member_rns_identity
          : undefined,
    cells: cells
      .filter((value): value is Record<string, unknown> => Boolean(value) && typeof value === "object")
      .map(toChecklistCellRecord),
  };
}

export function toChecklistFeedPublicationRecord(raw: Record<string, unknown>): ChecklistFeedPublicationRecord {
  return {
    publicationUid: String(raw.publicationUid ?? raw.publication_uid ?? ""),
    checklistUid: String(raw.checklistUid ?? raw.checklist_uid ?? ""),
    missionFeedUid: String(raw.missionFeedUid ?? raw.mission_feed_uid ?? ""),
    publishedAt: typeof raw.publishedAt === "string"
      ? raw.publishedAt
      : typeof raw.published_at === "string"
        ? raw.published_at
        : undefined,
    publishedByTeamMemberRnsIdentity:
      typeof raw.publishedByTeamMemberRnsIdentity === "string"
        ? raw.publishedByTeamMemberRnsIdentity
        : typeof raw.published_by_team_member_rns_identity === "string"
          ? raw.published_by_team_member_rns_identity
          : undefined,
  };
}

export function toChecklistRecord(raw: Record<string, unknown>): ChecklistRecord {
  const columns = Array.isArray(raw.columns) ? raw.columns : [];
  const tasks = Array.isArray(raw.tasks) ? raw.tasks : [];
  const feedPublications = Array.isArray(raw.feedPublications)
    ? raw.feedPublications
    : Array.isArray(raw.feed_publications)
      ? raw.feed_publications
      : [];
  const counts = raw.counts && typeof raw.counts === "object" ? raw.counts as Record<string, unknown> : {};
  return {
    uid: String(raw.uid ?? ""),
    missionUid: typeof raw.missionUid === "string" ? raw.missionUid : typeof raw.mission_uid === "string" ? raw.mission_uid : undefined,
    templateUid: typeof raw.templateUid === "string" ? raw.templateUid : typeof raw.template_uid === "string" ? raw.template_uid : undefined,
    templateVersion: toOptionalNumber(raw.templateVersion ?? raw.template_version),
    templateName: typeof raw.templateName === "string" ? raw.templateName : typeof raw.template_name === "string" ? raw.template_name : undefined,
    name: String(raw.name ?? ""),
    description: String(raw.description ?? ""),
    startTime: typeof raw.startTime === "string" ? raw.startTime : typeof raw.start_time === "string" ? raw.start_time : undefined,
    mode: String(raw.mode ?? "ONLINE") as ChecklistMode,
    syncState: String(raw.syncState ?? raw.sync_state ?? "SYNCED") as ChecklistSyncState,
    originType: String(raw.originType ?? raw.origin_type ?? "RCH_TEMPLATE") as ChecklistOriginType,
    checklistStatus: String(raw.checklistStatus ?? raw.checklist_status ?? "PENDING") as ChecklistTaskStatus,
    createdAt: typeof raw.createdAt === "string" ? raw.createdAt : typeof raw.created_at === "string" ? raw.created_at : undefined,
    createdByTeamMemberRnsIdentity: String(
      raw.createdByTeamMemberRnsIdentity ?? raw.created_by_team_member_rns_identity ?? "",
    ),
    createdByTeamMemberDisplayName: typeof raw.createdByTeamMemberDisplayName === "string"
      ? raw.createdByTeamMemberDisplayName
      : typeof raw.created_by_team_member_display_name === "string"
        ? raw.created_by_team_member_display_name
        : undefined,
    updatedAt: typeof raw.updatedAt === "string" ? raw.updatedAt : typeof raw.updated_at === "string" ? raw.updated_at : undefined,
    lastChangedByTeamMemberRnsIdentity: typeof raw.lastChangedByTeamMemberRnsIdentity === "string"
      ? raw.lastChangedByTeamMemberRnsIdentity
      : typeof raw.last_changed_by_team_member_rns_identity === "string"
        ? raw.last_changed_by_team_member_rns_identity
        : undefined,
    deletedAt: typeof raw.deletedAt === "string" ? raw.deletedAt : typeof raw.deleted_at === "string" ? raw.deleted_at : undefined,
    uploadedAt: typeof raw.uploadedAt === "string" ? raw.uploadedAt : typeof raw.uploaded_at === "string" ? raw.uploaded_at : undefined,
    participantRnsIdentities: Array.isArray(raw.participantRnsIdentities)
      ? raw.participantRnsIdentities.filter((value): value is string => typeof value === "string")
      : Array.isArray(raw.participant_rns_identities)
        ? raw.participant_rns_identities.filter((value): value is string => typeof value === "string")
        : [],
    expectedTaskCount: toOptionalNumber(raw.expectedTaskCount ?? raw.expected_task_count),
    progressPercent: Number(raw.progressPercent ?? raw.progress_percent ?? 0),
    counts: {
      pendingCount: Number(counts.pendingCount ?? counts.pending_count ?? 0),
      lateCount: Number(counts.lateCount ?? counts.late_count ?? 0),
      completeCount: Number(counts.completeCount ?? counts.complete_count ?? 0),
    },
    columns: columns
      .filter((value): value is Record<string, unknown> => Boolean(value) && typeof value === "object")
      .map(toChecklistColumnRecord),
    tasks: tasks
      .filter((value): value is Record<string, unknown> => Boolean(value) && typeof value === "object")
      .map(toChecklistTaskRecord),
    feedPublications: feedPublications
      .filter((value): value is Record<string, unknown> => Boolean(value) && typeof value === "object")
      .map(toChecklistFeedPublicationRecord),
  };
}

export function toChecklistTemplateRecord(raw: Record<string, unknown>): ChecklistTemplateRecord {
  const columns = Array.isArray(raw.columns) ? raw.columns : [];
  const tasks = Array.isArray(raw.tasks) ? raw.tasks : [];
  return {
    uid: String(raw.uid ?? ""),
    name: String(raw.name ?? ""),
    description: String(raw.description ?? ""),
    version: Number(raw.version ?? 1),
    originType: String(raw.originType ?? raw.origin_type ?? "RCH_TEMPLATE") as ChecklistOriginType,
    createdAt: typeof raw.createdAt === "string" ? raw.createdAt : typeof raw.created_at === "string" ? raw.created_at : undefined,
    updatedAt: typeof raw.updatedAt === "string" ? raw.updatedAt : typeof raw.updated_at === "string" ? raw.updated_at : undefined,
    sourceFilename: typeof raw.sourceFilename === "string"
      ? raw.sourceFilename
      : typeof raw.source_filename === "string"
        ? raw.source_filename
        : undefined,
    columns: columns
      .filter((value): value is Record<string, unknown> => Boolean(value) && typeof value === "object")
      .map(toChecklistColumnRecord),
    tasks: tasks
      .filter((value): value is Record<string, unknown> => Boolean(value) && typeof value === "object")
      .map(toChecklistTaskRecord),
  };
}
