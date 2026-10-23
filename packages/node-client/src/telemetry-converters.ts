import type { TelemetryPublicationSnapshot, TelemetryPublicationState } from "./contracts-telemetry";

const states = new Set<TelemetryPublicationState>([
  "StoredLocally", "QueuedNoRoute", "QueuedForDelivery", "Transmitted", "TransportDelivered",
  "Acknowledged", "Rejected", "TimedOut", "Failed",
]);

function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Invalid telemetry publication record");
  return value as Record<string, unknown>;
}
function text(value: unknown): string {
  if (typeof value !== "string") throw new Error("Invalid telemetry publication string");
  return value;
}
function number(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) throw new Error("Invalid telemetry publication timestamp");
  return value;
}
function optionalText(value: unknown): string | null { return value == null ? null : text(value); }

export function toTelemetryPublication(value: unknown): TelemetryPublicationSnapshot | null {
  if (value == null) return null;
  const raw = record(value);
  if (!Array.isArray(raw.recipients)) throw new Error("Invalid telemetry recipients");
  const failure = raw.last_receive_failure == null ? null : record(raw.last_receive_failure);
  return {
    localFixUpdatedAt: raw.local_fix_updated_at_ms == null ? null : number(raw.local_fix_updated_at_ms),
    callsign: optionalText(raw.callsign),
    recipients: raw.recipients.map((value) => {
      const row = record(value);
      const state = text(row.state) as TelemetryPublicationState;
      if (!states.has(state)) throw new Error("Unknown telemetry publication state");
      return {
        destinationHex: text(row.destination_hex), callsign: text(row.callsign),
        fixUpdatedAt: number(row.fix_updated_at_ms), commandId: text(row.command_id), state,
        messageIdHex: optionalText(row.message_id_hex), representation: optionalText(row.representation),
        detail: optionalText(row.detail), updatedAt: number(row.updated_at_ms),
      };
    }),
    lastReceiveFailure: failure ? {
      sourceHex: optionalText(failure.source_hex), destinationHex: text(failure.destination_hex),
      messageIdHex: optionalText(failure.message_id_hex), commandId: optionalText(failure.command_id),
      reason: text(failure.reason), receivedAt: number(failure.received_at_ms),
    } : null,
  };
}
