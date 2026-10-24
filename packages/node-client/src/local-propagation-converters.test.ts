import { describe, expect, it } from "vitest";
import { toLocalPropagationRecord, toLocalPropagationConfig } from "./local-propagation-converters";
import { classifyNodeError } from "./errors";
import { toLxmfDeliveryStatus } from "./message-converters";

const row = {
  transient_id_hex: "aa".repeat(32), message_id_hex: "bb".repeat(32),
  destination_hex: "cc".repeat(16), state: "StoredLocally", encrypted_bytes: 1024,
  admitted_at_ms: 1000, expires_at_ms: 2000, may_exist_on_relay: false,
};
describe("native local propagation boundary", () => {
  it("preserves all custody states and relay uncertainty", () => {
    for (const state of ["StoredLocally", "Uploading", "UploadedUnverified", "RelayHolds", "Delivered", "Acknowledged", "Expired", "Evicted"]) {
      expect(toLocalPropagationRecord({ ...row, state, may_exist_on_relay: true })).toMatchObject({
        state, encryptedBytes: 1024, mayExistOnRelay: true, messageIdHex: row.message_id_hex,
      });
      expect(toLxmfDeliveryStatus(state)).toBe(state);
    }
  });
  it("rejects malformed native projections", () => {
    for (const patch of [{ state: "Sent" }, { encrypted_bytes: -1 }, { expires_at_ms: NaN },
      { may_exist_on_relay: "true" }, { destination_hex: "" }, { message_id_hex: "ab" }]) {
      expect(() => toLocalPropagationRecord({ ...row, ...patch })).toThrow();
    }
  });
  it("decodes native limits without providing browser defaults", () => {
    expect(toLocalPropagationConfig({ capacity_bytes: 26214400, retention_seconds: 604800,
      lora_poll_interval_seconds: 300, lora_transfer_limit_bytes: 2048 })).toEqual({
      capacityBytes: 26214400, retentionSeconds: 604800,
      loraPollIntervalSeconds: 300, loraTransferLimitBytes: 2048,
    });
    expect(() => toLocalPropagationConfig({})).toThrow();
  });
  it("retains typed admission failures", () => {
    for (const code of ["UnresolvedRecipient", "PayloadTooLarge", "StoreFull", "Expired"]) {
      expect(classifyNodeError({ code }).code).toBe(code);
    }
  });
});
