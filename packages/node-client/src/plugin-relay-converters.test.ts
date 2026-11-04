import { describe, expect, it } from "vitest";
import { toPluginCapabilities } from "./runtime-converters";

describe("communication relay grant", () => {
  it("does not grant relay access to legacy receive plugins", () => {
    expect(toPluginCapabilities({ lxmfReceive: true }).communicationsRelay).toBe(false);
  });

  it("preserves the native relay grant in settings", () => {
    expect(toPluginCapabilities({ communicationsRelay: true }).communicationsRelay).toBe(true);
    expect(toPluginCapabilities({ communications_relay: true }).communicationsRelay).toBe(true);
  });
});
