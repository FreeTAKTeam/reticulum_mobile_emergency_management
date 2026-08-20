import { describe, expect, it } from "vitest";

import {
  normalizeRnodeSettings,
  parseRnodeConnectionMode,
  toAppSettingsRecord,
} from "./converters";

describe("RNode settings conversion", () => {
  it("defaults missing legacy modes to BLE", () => {
    expect(normalizeRnodeSettings({
      enabled: true,
      peripheral_id: " AA:BB ",
      display_name: " Field RNode ",
      region: "eu868",
      profile: "REM-MF-URBAN-v1",
    })).toEqual({
      enabled: true,
      connectionMode: "ble",
      peripheralId: "AA:BB",
      displayName: "Field RNode",
      region: "EU868",
      profile: "REM-MF-URBAN-v1",
      frequencyHz: 868_000_000,
    });
  });

  it("normalizes supported aliases and rejects unknown explicit modes", () => {
    expect(parseRnodeConnectionMode("Bluetooth Classic")).toBe("bluetooth_classic");
    expect(parseRnodeConnectionMode("serial")).toBe("usb");
    expect(parseRnodeConnectionMode("wifi")).toBe("tcp");
    expect(() => parseRnodeConnectionMode("carrier-pigeon")).toThrow(
      /Unsupported RNode connection mode/,
    );
  });

  it("rejects explicit unknown LoRa regions and profiles", () => {
    expect(() => normalizeRnodeSettings({ region: "XX000" })).toThrow(
      /Unsupported RNode LoRa region/,
    );
    expect(() => normalizeRnodeSettings({ profile: "REM-TYPO-v1" })).toThrow(
      /Unsupported RNode LoRa profile/,
    );
  });

  it("replaces out-of-range LoRa frequencies with the selected regional default", () => {
    expect(normalizeRnodeSettings({ region: "EU868", frequencyHz: 1 }).frequencyHz)
      .toBe(868_000_000);
    expect(normalizeRnodeSettings({ region: "US915", frequencyHz: 3_000_000_001 }).frequencyHz)
      .toBe(915_000_000);
    expect(normalizeRnodeSettings({ region: "US915", frequencyHz: 433_000_000 }).frequencyHz)
      .toBe(433_000_000);
  });
});

describe("app settings conversion", () => {
  it("normalizes nested plugin records without changing defaults", () => {
    expect(toAppSettingsRecord({
      settings: {
        displayName: "Atlas-9",
        tcpClients: ["mesh.example:4242"],
        telemetry: { accuracyThresholdMeters: "" },
        hub: { mode: "RchLxmf" },
        checklists: { defaultTaskDueStepMinutes: 0 },
        rnode: { mode: "gatt" },
      },
    })).toMatchObject({
      displayName: "Atlas-9",
      tcpClients: ["mesh.example:4242"],
      transportNodeEnabled: true,
      announceIntervalSeconds: 1800,
      telemetry: {
        accuracyThresholdMeters: undefined,
        publishIntervalSeconds: 360,
      },
      hub: {
        mode: "SemiAutonomous",
        refreshIntervalSeconds: 3600,
      },
      checklists: { defaultTaskDueStepMinutes: 1 },
      rnode: { connectionMode: "ble" },
    });
  });

  it("returns null for absent or malformed plugin settings", () => {
    expect(toAppSettingsRecord({})).toBeNull();
    expect(toAppSettingsRecord({ settings: "invalid" })).toBeNull();
  });

  it("bounds nested wrappers and fails closed for malformed nested sections", () => {
    const cyclic: Record<string, unknown> = {};
    cyclic.settings = cyclic;
    const coercionTrap = { toString: () => { throw new Error("unexpected coercion"); } };
    let deeplyNested: Record<string, unknown> = { displayName: "too deep" };
    for (let depth = 0; depth < 1_000; depth += 1) {
      deeplyNested = { settings: deeplyNested };
    }

    expect(toAppSettingsRecord(cyclic)).toBeNull();
    expect(toAppSettingsRecord(deeplyNested)).toBeNull();
    expect(toAppSettingsRecord({
      settings: {
        announceIntervalSeconds: "not-a-number",
        autoConnectSaved: "false",
        broadcast: "true",
        transportNodeEnabled: "false",
        tcpClients: [" mesh.example:4242 ", "mesh.example:4242", 42, ""],
        telemetry: null,
        hub: { mode: coercionTrap, identityHash: coercionTrap },
        checklists: null,
        rnode: {
          connectionMode: coercionTrap,
          peripheralId: coercionTrap,
          region: coercionTrap,
          profile: coercionTrap,
        },
      },
    })).toMatchObject({
      announceIntervalSeconds: 1800,
      autoConnectSaved: false,
      broadcast: false,
      transportNodeEnabled: true,
      tcpClients: ["mesh.example:4242"],
      telemetry: {
        publishIntervalSeconds: 360,
        accuracyThresholdMeters: undefined,
        staleAfterMinutes: 30,
        expireAfterMinutes: 180,
      },
      hub: {
        mode: "Autonomous",
        identityHash: "",
        refreshIntervalSeconds: 3600,
      },
      checklists: { defaultTaskDueStepMinutes: 30 },
      rnode: {
        enabled: false,
        connectionMode: "ble",
        peripheralId: "",
        region: "US915",
        profile: "REM-LF-RURAL-v1",
      },
    });
  });

  it("clamps numeric boundary values to valid runtime settings", () => {
    expect(toAppSettingsRecord({
      announceIntervalSeconds: 0,
      telemetry: {
        publishIntervalSeconds: 0,
        accuracyThresholdMeters: -1,
        staleAfterMinutes: 0,
        expireAfterMinutes: 0,
      },
      hub: { refreshIntervalSeconds: 0 },
      checklists: { defaultTaskDueStepMinutes: Number.POSITIVE_INFINITY },
    })).toMatchObject({
      announceIntervalSeconds: 60,
      telemetry: {
        publishIntervalSeconds: 1,
        accuracyThresholdMeters: 0,
        staleAfterMinutes: 1,
        expireAfterMinutes: 1,
      },
      hub: { refreshIntervalSeconds: 60 },
      checklists: { defaultTaskDueStepMinutes: 30 },
    });
  });
});
