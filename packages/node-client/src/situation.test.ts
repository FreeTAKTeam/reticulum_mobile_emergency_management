import { beforeEach, describe, expect, it, vi } from "vitest";

const plugin = vi.hoisted(() => ({
  addListener: vi.fn(),
  setSituation: vi.fn(),
  getHouseholdSituation: vi.fn(),
  setHouseholdMode: vi.fn(),
}));

vi.mock("./capacitor-plugin", () => ({
  ReticulumNodePluginInstance: plugin,
}));

import { CapacitorReticulumNodeClient } from "./capacitor-client";
import { WebReticulumNodeClient } from "./web-client";
import { toHouseholdSituationRecord } from "./situation-converters";

const NATIVE_RECORD = {
  situation: "need_supplies",
  detail: "water",
  mecpCode: "S01",
  eventUid: "rem-sit-v1:aa:3",
  sequence: 3,
  updatedAtMs: 1_700,
  householdMode: "this_phone_reports",
  householdId: "0123456789abcdef",
  situationEamFactors: ["preparedness"],
  heldByPowerSaver: true,
  duplicateReporter: {
    sourceIdentity: "bb",
    householdName: "Harbour House",
    updatedAtMs: 1_600,
  },
  messages: [
    { kind: "community_status", outcome: "queued", reference: "needs_help" },
    { kind: "event", outcome: "held_by_power_saver", reference: "rem-sit-v1:aa:3" },
    { kind: "eam", outcome: "held_by_power_saver", reference: "Pixel" },
  ],
};

describe("household situation contract", () => {
  beforeEach(() => {
    plugin.addListener.mockReset();
    plugin.addListener.mockResolvedValue({ remove: vi.fn().mockResolvedValue(undefined) });
    plugin.setSituation.mockReset();
    plugin.getHouseholdSituation.mockReset();
    plugin.setHouseholdMode.mockReset();
  });

  it("maps the native record into typed fields", () => {
    const record = toHouseholdSituationRecord(NATIVE_RECORD);
    expect(record.situation).toBe("need_supplies");
    expect(record.householdMode).toBe("this_phone_reports");
    expect(record.duplicateReporter?.householdName).toBe("Harbour House");
    expect(record.messages.map((message) => message.outcome)).toEqual([
      "queued",
      "held_by_power_saver",
      "held_by_power_saver",
    ]);
  });

  it("drops unknown enum values instead of guessing", () => {
    const record = toHouseholdSituationRecord({
      situation: "panic",
      householdMode: "maybe",
      messages: [{ kind: "fax", outcome: "lost" }],
    });
    expect(record.situation).toBeUndefined();
    expect(record.householdMode).toBe("this_phone_reports");
    expect(record.messages[0]).toEqual({ kind: "event", outcome: "failed", reference: undefined });
  });

  it("sends only the choice to the native plugin", async () => {
    plugin.setSituation.mockResolvedValue(NATIVE_RECORD);
    plugin.setHouseholdMode.mockResolvedValue({ ...NATIVE_RECORD, householdMode: "off" });
    plugin.getHouseholdSituation.mockResolvedValue(NATIVE_RECORD);
    const client = new CapacitorReticulumNodeClient();

    await expect(client.setSituation("need_supplies", "water")).resolves.toMatchObject({
      mecpCode: "S01",
    });
    expect(plugin.setSituation).toHaveBeenCalledWith({ situation: "need_supplies", detail: "water" });
    await expect(client.setHouseholdMode("off")).resolves.toMatchObject({ householdMode: "off" });
    expect(plugin.setHouseholdMode).toHaveBeenCalledWith({ mode: "off" });
    await expect(client.getHouseholdSituation()).resolves.toMatchObject({ sequence: 3 });
    await client.dispose();
  });

  it("leaves situations to the native runtime in web mode", async () => {
    const client = new WebReticulumNodeClient();
    await expect(client.getHouseholdSituation()).resolves.toMatchObject({
      householdMode: "this_phone_reports",
      messages: [],
    });
    await expect(client.setSituation("all_ok")).rejects.toThrow("native runtime");
  });
});
