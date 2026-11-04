import { App } from "@capacitor/app";
import { Capacitor } from "@capacitor/core";
import { beforeEach, expect, test, vi } from "vitest";

import { shutdownRem } from "./shutdownRem";

vi.mock("@capacitor/app", () => ({ App: { exitApp: vi.fn() } }));
vi.mock("@capacitor/core", () => ({ Capacitor: { getPlatform: vi.fn() } }));

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(Capacitor.getPlatform).mockReturnValue("android");
});

test("Android closes only after the runtime and background service stop", async () => {
  let finishStop = () => {};
  const stopping = new Promise<void>((resolve) => { finishStop = resolve; });
  const stop = vi.fn(() => stopping);
  const shutdown = shutdownRem(stop);
  expect(stop).toHaveBeenCalledOnce();
  expect(App.exitApp).not.toHaveBeenCalled();
  finishStop();
  await shutdown;
  expect(App.exitApp).toHaveBeenCalledOnce();
});

test("failed stop leaves the Android app open", async () => {
  const stop = vi.fn().mockRejectedValue(new Error("Native stop failed"));
  await expect(shutdownRem(stop)).rejects.toThrow("Native stop failed");
  expect(App.exitApp).not.toHaveBeenCalled();
});

test("browser stops the runtime without trying to close the tab", async () => {
  vi.mocked(Capacitor.getPlatform).mockReturnValue("web");
  const stop = vi.fn().mockResolvedValue(undefined);
  await shutdownRem(stop);
  expect(stop).toHaveBeenCalledOnce();
  expect(App.exitApp).not.toHaveBeenCalled();
});

test("a failed Android close remains visible to the caller", async () => {
  vi.mocked(App.exitApp).mockRejectedValue(new Error("Activity close failed"));
  const stop = vi.fn().mockResolvedValue(undefined);
  await expect(shutdownRem(stop)).rejects.toThrow("Activity close failed");
  expect(stop).toHaveBeenCalledOnce();
});
