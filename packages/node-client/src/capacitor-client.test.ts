import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const plugin = vi.hoisted(() => ({
  addListener: vi.fn(),
  getStatus: vi.fn(),
}));

vi.mock("./capacitor-plugin", () => ({
  ReticulumNodePluginInstance: plugin,
}));

import { CapacitorReticulumNodeClient } from "./capacitor-client";

describe("CapacitorReticulumNodeClient listener setup", () => {
  beforeEach(() => {
    plugin.addListener.mockReset();
    plugin.getStatus.mockReset();
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
});
