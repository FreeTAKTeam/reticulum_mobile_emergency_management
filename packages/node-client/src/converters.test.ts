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
        accuracyThresholdMeters: 0,
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
});
