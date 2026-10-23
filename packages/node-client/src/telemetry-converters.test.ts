import { describe, expect, it } from "vitest";
import { toTelemetryPublication } from "./telemetry-converters";

describe("telemetry publication projection", () => {
  it("preserves transport delivery separately from receiver acknowledgement", () => {
    const result = toTelemetryPublication({
      local_fix_updated_at_ms: 123, callsign: "Alpha", last_receive_failure: null,
      recipients: [{ destination_hex: "a".repeat(32), callsign: "Alpha", fix_updated_at_ms: 123,
        command_id: "t:1", state: "TransportDelivered", message_id_hex: "b".repeat(64),
        representation: "Packet", detail: null, updated_at_ms: 125 }],
    });
    expect(result?.recipients[0].state).toBe("TransportDelivered");
    expect(result?.recipients[0].commandId).toBe("t:1");
  });
  it("keeps unavailable old-native snapshots explicit and rejects incompatible data", () => {
    expect(toTelemetryPublication(undefined)).toBeNull();
    expect(() => toTelemetryPublication({ recipients: "accepted" })).toThrow();
    expect(() => toTelemetryPublication({ recipients: [{ state: "remote-success" }] })).toThrow();
  });
});
