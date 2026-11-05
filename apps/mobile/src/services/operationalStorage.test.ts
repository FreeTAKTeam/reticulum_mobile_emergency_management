import { Capacitor } from "@capacitor/core";
import { beforeEach, expect, test, vi } from "vitest";
import { operationalStorage } from "./operationalStorage";

vi.mock("@capacitor/core", async (importOriginal) => ({
  ...await importOriginal<typeof import("@capacitor/core")>(),
  Capacitor: { getPlatform: vi.fn() },
}));

beforeEach(() => {
  vi.stubGlobal("localStorage", {
    getItem: vi.fn(() => "retired sensitive cache"),
    setItem: vi.fn(),
    removeItem: vi.fn(),
  });
  vi.mocked(Capacitor.getPlatform).mockReturnValue("web");
});

test("browser operational caches neither read nor write durable storage", async () => {
  const key = "reticulum.mobile.inbox.v1";
  expect(operationalStorage.getItem(key)).toBeNull();
  operationalStorage.setItem(key, "new message");
  expect(operationalStorage.getItem(key)).toBe("new message");
  vi.resetModules();
  const reloaded = await import("./operationalStorage");
  expect(reloaded.operationalStorage.getItem(key)).toBeNull();
  operationalStorage.removeItem(key);
  expect(operationalStorage.getItem(key)).toBeNull();
  expect(localStorage.getItem).not.toHaveBeenCalled();
  expect(localStorage.setItem).not.toHaveBeenCalled();
  expect(localStorage.removeItem).not.toHaveBeenCalled();
});

test("native WebView legacy storage remains available for verified runtime migration", () => {
  vi.mocked(Capacitor.getPlatform).mockReturnValue("android");
  expect(operationalStorage.getItem("legacy")).toBe("retired sensitive cache");
  operationalStorage.setItem("legacy", "payload");
  operationalStorage.removeItem("legacy");
  expect(localStorage.setItem).toHaveBeenCalledWith("legacy", "payload");
  expect(localStorage.removeItem).toHaveBeenCalledWith("legacy");
});

test("legacy browser settings cannot retain or restore hub credentials; native import retains them", async () => {
  const { persistWebLegacySettings } = await import("../utils/legacyStorage");
  const { buildLegacyProjectionState } = await import("../utils/legacyState");
  const { DEFAULT_SETTINGS } = await import("../stores/nodeSettingsModel");
  const settings = { ...DEFAULT_SETTINGS, hub: { ...DEFAULT_SETTINGS.hub, apiKey: "private-key" } };
  persistWebLegacySettings(settings);
  expect(settings.hub.apiKey).toBe("private-key");
  const saved = vi.mocked(localStorage.setItem).mock.calls[0][1];
  expect(JSON.parse(saved).hub.apiKey).toBe("");
  vi.mocked(localStorage.getItem).mockImplementation((key) =>
    key === "reticulum.mobile.settings.v1" ? JSON.stringify(settings) : null);
  expect(buildLegacyProjectionState(DEFAULT_SETTINGS)?.payload.settings?.hub.apiKey).toBe("");
  vi.mocked(Capacitor.getPlatform).mockReturnValue("android");
  expect(buildLegacyProjectionState(DEFAULT_SETTINGS)?.payload.settings?.hub.apiKey).toBe("private-key");
});

test("native legacy operational records remain importable while retired browser records are ignored", async () => {
  const { buildLegacyProjectionState } = await import("../utils/legacyState");
  const { DEFAULT_SETTINGS } = await import("../stores/nodeSettingsModel");
  vi.mocked(localStorage.getItem).mockImplementation((key) => key === "reticulum.mobile.savedPeers.v1"
    ? JSON.stringify([{ destination: "a".repeat(32), label: "Rescue", savedAt: 1 }]) : null);
  expect(buildLegacyProjectionState(DEFAULT_SETTINGS)).toBeNull();
  vi.mocked(Capacitor.getPlatform).mockReturnValue("android");
  expect(buildLegacyProjectionState(DEFAULT_SETTINGS)?.payload.savedPeers).toEqual([
    { destination: "a".repeat(32), label: "Rescue", savedAt: 1, circleTier: "inner" },
  ]);
  expect(localStorage.removeItem).not.toHaveBeenCalled();
});
