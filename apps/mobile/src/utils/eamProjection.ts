import type {
  EamProjectionRecord,
  EamReadinessSummaryRecord,
  EamTeamSummaryRecord,
} from "@reticulum/node-client";

import type { ActionMessage, EamStatus, EamTeamSummary, EamWireStatus } from "../types/domain";
import { DEFAULT_R3AKT_TEAM_COLOR, normalizeR3aktTeamColor } from "./r3akt";

const MESSAGE_STORAGE_KEY = "reticulum.mobile.messages.v1";

export type StoredMessages = Record<string, ActionMessage>;

export function emptyEamReadinessSummary(): EamReadinessSummaryRecord {
  return {
    activeTotal: 0,
    updatedAt: 0,
    statusMetrics: [],
    messages: [],
  };
}

export function nextLocalUpdatedAt(previousUpdatedAt?: number): number {
  const now = Date.now();
  if (typeof previousUpdatedAt !== "number" || !Number.isFinite(previousUpdatedAt)) {
    return now;
  }
  return Math.max(now, previousUpdatedAt + 1);
}

function normalizeStatus(value: unknown): EamStatus {
  return value === "Green" || value === "Yellow" || value === "Red" ? value : "Unknown";
}

function normalizeWireStatus(value: unknown): EamWireStatus | undefined {
  return value === "Green" || value === "Yellow" || value === "Red" ? value : undefined;
}

function normalizeSyncState(value: unknown): ActionMessage["syncState"] {
  return value === "draft" || value === "syncing" || value === "synced" || value === "error"
    ? value
    : undefined;
}

export function optionalNumber(value: unknown): number | undefined {
  const numeric = Number(value);
  return Number.isFinite(numeric) ? numeric : undefined;
}

function asTrimmedString(value: unknown): string | undefined {
  return typeof value === "string" ? value.trim() || undefined : undefined;
}

export function normalizeIdentifier(value: unknown): string {
  return typeof value === "string" ? value.trim().toLowerCase() : "";
}

export function keyFor(callsign: string): string {
  return callsign.trim().toLowerCase();
}

export function cloneMessage(message: ActionMessage): ActionMessage {
  return {
    ...message,
    source: message.source ? { ...message.source } : undefined,
  };
}

export function normalizeMessage(entry: Partial<ActionMessage>): ActionMessage {
  const updatedAt = Number(entry.updatedAt ?? Date.now());
  return {
    callsign: String(entry.callsign ?? "").trim(),
    groupName: normalizeR3aktTeamColor(entry.groupName, DEFAULT_R3AKT_TEAM_COLOR),
    securityStatus: normalizeStatus(entry.securityStatus),
    capabilityStatus: normalizeStatus(entry.capabilityStatus),
    preparednessStatus: normalizeStatus(entry.preparednessStatus),
    medicalStatus: normalizeStatus(entry.medicalStatus),
    mobilityStatus: normalizeStatus(entry.mobilityStatus),
    commsStatus: normalizeStatus(entry.commsStatus),
    notes: asTrimmedString(entry.notes),
    updatedAt: Number.isFinite(updatedAt) ? updatedAt : Date.now(),
    deletedAt: optionalNumber(entry.deletedAt),
    eamUid: asTrimmedString(entry.eamUid),
    teamMemberUid: asTrimmedString(entry.teamMemberUid),
    teamUid: asTrimmedString(entry.teamUid),
    reportedAt: asTrimmedString(entry.reportedAt),
    reportedBy: asTrimmedString(entry.reportedBy),
    overallStatus: normalizeWireStatus(entry.overallStatus),
    confidence: optionalNumber(entry.confidence),
    ttlSeconds: optionalNumber(entry.ttlSeconds),
    source:
      entry.source && typeof entry.source === "object" && !Array.isArray(entry.source)
        ? {
            rns_identity: String((entry.source as { rns_identity?: unknown }).rns_identity ?? "").trim(),
            display_name: asTrimmedString((entry.source as { display_name?: unknown }).display_name),
          }
        : undefined,
    syncState: normalizeSyncState(entry.syncState),
    syncError: asTrimmedString(entry.syncError),
    draftCreatedAt: optionalNumber(entry.draftCreatedAt),
    lastSyncedAt: optionalNumber(entry.lastSyncedAt),
  };
}

export function loadWebMessages(): StoredMessages {
  try {
    const raw = localStorage.getItem(MESSAGE_STORAGE_KEY);
    if (!raw) {
      return {};
    }
    const parsed = JSON.parse(raw) as Array<Partial<ActionMessage>>;
    const out: StoredMessages = {};
    for (const entry of parsed) {
      const normalized = normalizeMessage(entry);
      if (!normalized.callsign) {
        continue;
      }
      out[keyFor(normalized.callsign)] = normalized;
    }
    return out;
  } catch {
    return {};
  }
}

export function saveWebMessages(messages: StoredMessages): void {
  localStorage.setItem(MESSAGE_STORAGE_KEY, JSON.stringify(Object.values(messages)));
}

export function toProjectionRecord(message: ActionMessage): EamProjectionRecord {
  return {
    callsign: message.callsign,
    groupName: normalizeR3aktTeamColor(message.groupName, DEFAULT_R3AKT_TEAM_COLOR),
    securityStatus: message.securityStatus,
    capabilityStatus: message.capabilityStatus,
    preparednessStatus: message.preparednessStatus,
    medicalStatus: message.medicalStatus,
    mobilityStatus: message.mobilityStatus,
    commsStatus: message.commsStatus,
    notes: message.notes,
    updatedAt: message.updatedAt,
    deletedAt: message.deletedAt,
    eamUid: message.eamUid,
    teamMemberUid: message.teamMemberUid,
    teamUid: message.teamUid,
    reportedAt: message.reportedAt,
    reportedBy: message.reportedBy,
    overallStatus: message.overallStatus,
    confidence: message.confidence,
    ttlSeconds: message.ttlSeconds,
    source: message.source,
    syncState: message.syncState,
    syncError: message.syncError,
    draftCreatedAt: message.draftCreatedAt,
    lastSyncedAt: message.lastSyncedAt,
  };
}

function fromProjectionRecord(record: EamProjectionRecord): ActionMessage {
  return normalizeMessage({
    ...record,
    securityStatus: normalizeStatus(record.securityStatus),
    capabilityStatus: normalizeStatus(record.capabilityStatus),
    preparednessStatus: normalizeStatus(record.preparednessStatus),
    medicalStatus: normalizeStatus(record.medicalStatus),
    mobilityStatus: normalizeStatus(record.mobilityStatus),
    commsStatus: normalizeStatus(record.commsStatus),
    overallStatus: normalizeWireStatus(record.overallStatus),
    syncState: normalizeSyncState(record.syncState),
  });
}

export function toStoredMessages(records: EamProjectionRecord[]): StoredMessages {
  const out: StoredMessages = {};
  for (const record of records) {
    const message = fromProjectionRecord(record);
    if (message.callsign) {
      out[keyFor(message.callsign)] = message;
    }
  }
  return out;
}

export function toTeamSummary(record: EamTeamSummaryRecord | null): EamTeamSummary | null {
  if (!record) {
    return null;
  }
  const byStatus: Partial<Record<EamWireStatus, number>> = {};
  if (record.greenTotal > 0) byStatus.Green = record.greenTotal;
  if (record.yellowTotal > 0) byStatus.Yellow = record.yellowTotal;
  if (record.redTotal > 0) byStatus.Red = record.redTotal;
  return {
    team_uid: record.teamUid,
    total: record.total,
    active_total: record.activeTotal,
    deleted_total: record.deletedTotal,
    overall_status: normalizeWireStatus(record.overallStatus),
    by_status: byStatus,
    updated_at: new Date(record.updatedAt).toISOString(),
  };
}

export function countRedStatuses(message: ActionMessage): number {
  return [
    message.securityStatus,
    message.capabilityStatus,
    message.preparednessStatus,
    message.medicalStatus,
    message.mobilityStatus,
    message.commsStatus,
  ].filter((status) => status === "Red").length;
}
