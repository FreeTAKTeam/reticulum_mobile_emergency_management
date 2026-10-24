import type { LocalPropagationConfig, LocalPropagationRecord, LocalPropagationState } from "./contracts-local-propagation";

const states = new Set<LocalPropagationState>([
  "StoredLocally", "Uploading", "UploadedUnverified", "RelayHolds",
  "Delivered", "Acknowledged", "Expired", "Evicted",
]);
function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Invalid local propagation record");
  return value as Record<string, unknown>;
}
function integer(value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) throw new Error("Invalid local propagation number");
  return value;
}
function hash(value: unknown, length: number): string {
  if (typeof value !== "string" || value.length !== length || !/^[a-f0-9]+$/i.test(value)) throw new Error("Invalid local propagation hash");
  return value.toLowerCase();
}
export function toLocalPropagationRecord(value: unknown): LocalPropagationRecord {
  const row = object(value);
  const state = row.state as LocalPropagationState;
  if (!states.has(state) || typeof row.may_exist_on_relay !== "boolean") throw new Error("Invalid local propagation state");
  return {
    transientIdHex: hash(row.transient_id_hex, 64), messageIdHex: hash(row.message_id_hex, 64),
    destinationHex: hash(row.destination_hex, 32), state,
    encryptedBytes: integer(row.encrypted_bytes), admittedAtMs: integer(row.admitted_at_ms),
    expiresAtMs: integer(row.expires_at_ms), mayExistOnRelay: row.may_exist_on_relay,
  };
}
export function toLocalPropagationConfig(value: unknown): LocalPropagationConfig {
  const row = object(value);
  return {
    capacityBytes: integer(row.capacity_bytes), retentionSeconds: integer(row.retention_seconds),
    loraPollIntervalSeconds: integer(row.lora_poll_interval_seconds),
    loraTransferLimitBytes: integer(row.lora_transfer_limit_bytes),
  };
}
