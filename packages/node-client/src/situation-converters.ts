import type {
  HouseholdMode,
  HouseholdReporterConflictRecord,
  HouseholdSituation,
  HouseholdSituationRecord,
  SituationMessageKind,
  SituationMessageOutcome,
  SituationMessageRecord,
} from "./contracts";

const HOUSEHOLD_SITUATIONS: readonly HouseholdSituation[] = [
  "all_ok",
  "sheltering_in_place",
  "evacuating",
  "someone_missing",
  "lost",
  "stranded",
  "threat_nearby",
  "need_supplies",
  "medical_help_needed",
  "sos",
];
const HOUSEHOLD_MODES: readonly HouseholdMode[] = ["off", "this_phone_reports", "another_phone_reports"];
const MESSAGE_KINDS: readonly SituationMessageKind[] = ["community_status", "event", "eam", "sos"];
const MESSAGE_OUTCOMES: readonly SituationMessageOutcome[] = [
  "queued",
  "held_by_power_saver",
  "skipped_unchanged",
  "skipped_household_off",
  "skipped_another_phone_reports",
  "failed",
  "waiting_for_sos_countdown",
];

function oneOf<T extends string>(values: readonly T[], raw: unknown): T | undefined {
  return values.find((value) => value === raw);
}

function optionalString(raw: unknown): string | undefined {
  return typeof raw === "string" && raw.length > 0 ? raw : undefined;
}

function pick(raw: Record<string, unknown>, camel: string, snake: string): unknown {
  return raw[camel] ?? raw[snake];
}

function toSituationMessageRecord(raw: Record<string, unknown>): SituationMessageRecord {
  return {
    kind: oneOf(MESSAGE_KINDS, raw.kind) ?? "event",
    outcome: oneOf(MESSAGE_OUTCOMES, raw.outcome) ?? "failed",
    reference: optionalString(raw.reference),
  };
}

function toReporterConflict(raw: unknown): HouseholdReporterConflictRecord | undefined {
  if (!raw || typeof raw !== "object") {
    return undefined;
  }
  const record = raw as Record<string, unknown>;
  return {
    sourceIdentity: String(pick(record, "sourceIdentity", "source_identity") ?? ""),
    householdName: String(pick(record, "householdName", "household_name") ?? ""),
    updatedAtMs: Number(pick(record, "updatedAtMs", "updated_at_ms") ?? 0),
  };
}

/** Maps the native situation JSON; unknown enum values are dropped, never guessed. */
export function toHouseholdSituationRecord(raw: Record<string, unknown>): HouseholdSituationRecord {
  const messages = raw.messages;
  const factors = pick(raw, "situationEamFactors", "situation_eam_factors");
  return {
    situation: oneOf(HOUSEHOLD_SITUATIONS, raw.situation),
    detail: optionalString(raw.detail),
    mecpCode: optionalString(pick(raw, "mecpCode", "mecp_code")),
    eventUid: optionalString(pick(raw, "eventUid", "event_uid")),
    sequence: Number(raw.sequence ?? 0),
    updatedAtMs: Number(pick(raw, "updatedAtMs", "updated_at_ms") ?? 0),
    householdMode: oneOf(HOUSEHOLD_MODES, pick(raw, "householdMode", "household_mode")) ?? "this_phone_reports",
    householdId: optionalString(pick(raw, "householdId", "household_id")),
    situationEamFactors: Array.isArray(factors) ? factors.map(String) : [],
    heldByPowerSaver: Boolean(pick(raw, "heldByPowerSaver", "held_by_power_saver")),
    duplicateReporter: toReporterConflict(pick(raw, "duplicateReporter", "duplicate_reporter")),
    messages: Array.isArray(messages)
      ? messages.map((message) => toSituationMessageRecord(message as Record<string, unknown>))
      : [],
  };
}

export function emptyHouseholdSituation(): HouseholdSituationRecord {
  return {
    sequence: 0,
    updatedAtMs: 0,
    householdMode: "this_phone_reports",
    situationEamFactors: [],
    heldByPowerSaver: false,
    messages: [],
  };
}
