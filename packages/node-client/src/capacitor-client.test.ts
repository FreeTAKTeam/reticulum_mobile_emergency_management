import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { readFileSync } from "node:fs";

const BLOCK_ONBOARDING_FIXED_VECTOR = readFileSync(
  new URL("../../../apps/mobile/android/app/src/test/resources/block-onboarding-max-v1.txt", import.meta.url),
  "utf8",
);

const plugin = vi.hoisted(() => ({
  addListener: vi.fn(),
  getStatus: vi.fn(),
  getPowerState: vi.fn(),
  createBlockOnboardingCode: vi.fn(),
  inspectBlockOnboardingCode: vi.fn(),
}));

vi.mock("./capacitor-plugin", () => ({
  ReticulumNodePluginInstance: plugin,
}));

import { CapacitorReticulumNodeClient } from "./capacitor-client";

describe("CapacitorReticulumNodeClient listener setup", () => {
  beforeEach(() => {
    plugin.addListener.mockReset();
    plugin.getStatus.mockReset();
    plugin.getPowerState.mockReset();
    plugin.createBlockOnboardingCode.mockReset();
    plugin.inspectBlockOnboardingCode.mockReset();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("removes partially registered listeners before retrying", async () => {
    const partialRemove = vi.fn().mockResolvedValue(undefined);
    plugin.addListener
      .mockResolvedValueOnce({ remove: partialRemove })
      .mockRejectedValueOnce(new Error("listener unavailable"));

    const client = new CapacitorReticulumNodeClient();
    client.on("statusChanged", () => undefined);
    await expect(client.getStatus()).rejects.toMatchObject({
      message: "listener unavailable",
    });
    expect(partialRemove).toHaveBeenCalledOnce();

    const retryRemoves: Array<ReturnType<typeof vi.fn>> = [];
    plugin.addListener.mockImplementation(async () => {
      const remove = vi.fn().mockResolvedValue(undefined);
      retryRemoves.push(remove);
      return { remove };
    });
    plugin.getStatus.mockResolvedValue({ running: false });

    await expect(client.getStatus()).resolves.toMatchObject({ running: false });
    expect(retryRemoves.length).toBeGreaterThan(0);

    await client.dispose();
    expect(retryRemoves.every((remove) => remove.mock.calls.length === 1)).toBe(true);
  });

  it("ignores stale native callbacks when listener removal fails", async () => {
    const callbacks: Array<{
      eventName: string;
      callback: (payload: unknown) => void;
    }> = [];
    const remove = vi.fn().mockRejectedValue(new Error("remove unavailable"));
    plugin.addListener.mockImplementation(async (eventName, callback) => {
      callbacks.push({ eventName, callback });
      return { remove };
    });
    plugin.getStatus.mockResolvedValue({ running: false });
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);

    const client = new CapacitorReticulumNodeClient();
    client.on("statusChanged", () => undefined);
    await client.getStatus();
    const staleCallback = callbacks.find(({ eventName }) => eventName === "statusChanged")?.callback;
    expect(staleCallback).toBeDefined();

    await client.dispose();
    let received = 0;
    client.on("statusChanged", () => {
      received += 1;
    });
    await client.getStatus();
    const currentCallback = callbacks
      .filter(({ eventName }) => eventName === "statusChanged")
      .at(-1)?.callback;
    expect(currentCallback).toBeDefined();

    staleCallback?.({ status: { running: true } });
    expect(received).toBe(0);
    currentCallback?.({ status: { running: true } });
    expect(received).toBe(1);
    expect(warn).toHaveBeenCalled();
  });

  it("projects native power state and events", async () => {
    const callbacks = new Map<string, (payload: unknown) => void>();
    plugin.addListener.mockImplementation(async (eventName, callback) => {
      callbacks.set(eventName, callback);
      return { remove: vi.fn().mockResolvedValue(undefined) };
    });
    plugin.getPowerState.mockResolvedValue({
      batteryPercent: 19,
      charging: false,
      saverActive: true,
      updatedAtMs: 42,
    });

    const client = new CapacitorReticulumNodeClient();
    let eventState;
    client.on("powerStateChanged", (state) => {
      eventState = state;
    });

    await expect(client.getPowerState()).resolves.toEqual({
      batteryPercent: 19,
      charging: false,
      saverActive: true,
      updatedAtMs: 42,
    });
    callbacks.get("powerStateChanged")?.({
      batteryPercent: 31,
      charging: true,
      saverActive: false,
      updatedAtMs: 43,
    });
    expect(eventState).toEqual({
      batteryPercent: 31,
      charging: true,
      saverActive: false,
      updatedAtMs: 43,
    });
  });

  it("delegates Block Code signing and verification to the native plugin", async () => {
    plugin.addListener.mockResolvedValue({ remove: vi.fn().mockResolvedValue(undefined) });
    plugin.createBlockOnboardingCode.mockResolvedValue({ encodedText: "REMBC1:native" });
    plugin.inspectBlockOnboardingCode.mockResolvedValue({
      issuerPublicIdentityHex: "aa",
      issuerAppDestinationHex: "bb",
      issuerLxmfDestinationHex: "cc",
      signerFingerprint: "dd",
      issuedAtMs: 1,
      expiresAtMs: 2,
      network: {
        tcpClients: [],
        broadcast: true,
        hubMode: "Autonomous",
        hubRefreshIntervalSeconds: 3600,
      },
      trustedDestinationHashes: [],
      preferredMapLayer: "base",
    });
    const client = new CapacitorReticulumNodeClient();
    const draft = {
      network: {
        tcpClients: [],
        broadcast: true,
        hubMode: "Autonomous" as const,
        hubRefreshIntervalSeconds: 3600,
      },
      trustedDestinationHashes: [],
      preferredMapLayer: "base" as const,
      expiresAtMs: 2,
    };

    await expect(client.createBlockOnboardingCode(draft)).resolves.toEqual({
      encodedText: "REMBC1:native",
    });
    await expect(client.inspectBlockOnboardingCode("REMBC1:native")).resolves.toMatchObject({
      signerFingerprint: "dd",
    });
    expect(plugin.createBlockOnboardingCode).toHaveBeenCalledWith({ draft });
    expect(plugin.inspectBlockOnboardingCode).toHaveBeenCalledWith({
      encodedText: "REMBC1:native",
    });
  });

  it("passes the checked-in maximum signed vector opaquely to native verification", async () => {
    plugin.addListener.mockResolvedValue({ remove: vi.fn().mockResolvedValue(undefined) });
    plugin.inspectBlockOnboardingCode.mockResolvedValue({
      issuerPublicIdentityHex: "aa",
      issuerAppDestinationHex: "bb",
      issuerLxmfDestinationHex: "cc",
      signerFingerprint: "dd",
      issuedAtMs: 1,
      expiresAtMs: 2,
      network: { tcpClients: [], broadcast: true, hubMode: "Autonomous", hubRefreshIntervalSeconds: 3600 },
      trustedDestinationHashes: [],
      preferredMapLayer: "base",
    });
    expect(Buffer.byteLength(BLOCK_ONBOARDING_FIXED_VECTOR, "utf8")).toBe(1_999);

    const client = new CapacitorReticulumNodeClient();
    await client.inspectBlockOnboardingCode(BLOCK_ONBOARDING_FIXED_VECTOR);

    expect(plugin.inspectBlockOnboardingCode).toHaveBeenCalledWith({
      encodedText: BLOCK_ONBOARDING_FIXED_VECTOR,
    });
  });
});
