import { beforeEach, describe, expect, it, vi } from "vitest";

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
});
