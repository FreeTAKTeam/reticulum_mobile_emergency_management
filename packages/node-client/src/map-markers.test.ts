import { describe, expect, it, vi } from "vitest";

const plugin = vi.hoisted(() => ({
  addListener: vi.fn().mockResolvedValue({ remove: vi.fn().mockResolvedValue(undefined) }),
  listMapMarkerSymbols: vi.fn(),
  listMapMarkers: vi.fn(),
  createMapMarker: vi.fn(),
  renameMapMarker: vi.fn(),
  moveMapMarker: vi.fn(),
  deleteMapMarker: vi.fn(),
}));

vi.mock("./capacitor-plugin", () => ({
  ReticulumNodePluginInstance: plugin,
}));

import { CapacitorReticulumNodeClient } from "./capacitor-client";
import { toMapMarkerRecord, toMapMarkerSymbolRecord } from "./map-marker-converters";
import { MockReticulumNodeClient } from "./mock-client";

const nativeMarker = {
  objectDestinationHash: "a1b2",
  originRch: "",
  markerType: "fire",
  symbol: "fire",
  name: "Ridge",
  category: "incident",
  lat: 44.5,
  lon: -1.25,
  notes: null,
  createdAtMs: 10,
  updatedAtMs: 20,
};

describe("map marker converters", () => {
  it("maps native camelCase records and drops null notes", () => {
    expect(toMapMarkerRecord(nativeMarker)).toEqual({
      ...nativeMarker,
      notes: undefined,
    });
  });

  it("keeps only string aliases from the native catalog", () => {
    const symbol = toMapMarkerSymbolRecord({
      id: "fire",
      set: "mdi",
      mdi: "fire",
      description: "Fire",
      tak: null,
      category: "incident",
      aliases: ["flame", 7, "wildfire"],
    });
    expect(symbol.aliases).toEqual(["flame", "wildfire"]);
    expect(symbol.tak).toBeUndefined();
  });
});

describe("Capacitor map marker client", () => {
  it("forwards requests to native and converts results", async () => {
    plugin.createMapMarker.mockResolvedValue({ marker: nativeMarker, created: false });
    plugin.moveMapMarker.mockResolvedValue({ marker: { ...nativeMarker, lat: 1 } });
    plugin.deleteMapMarker.mockResolvedValue({ deleted: true });
    plugin.listMapMarkers.mockResolvedValue({ items: [nativeMarker, null] });
    const client = new CapacitorReticulumNodeClient();

    const created = await client.createMapMarker({
      markerType: "fire",
      symbol: "fire",
      lat: 44.5,
      lon: -1.25,
      idempotencyKey: "k-1",
    });
    expect(created.created).toBe(false);
    expect(plugin.createMapMarker).toHaveBeenCalledWith({
      request: expect.objectContaining({ idempotencyKey: "k-1", symbol: "fire" }),
    });
    await expect(client.moveMapMarker("a1b2", 1, -1.25)).resolves.toMatchObject({ lat: 1 });
    expect(plugin.moveMapMarker).toHaveBeenCalledWith({
      request: { objectDestinationHash: "a1b2", lat: 1, lon: -1.25 },
    });
    await expect(client.deleteMapMarker("a1b2")).resolves.toBe(true);
    await expect(client.listMapMarkers()).resolves.toHaveLength(1);
    await client.dispose();
  });
});

describe("in-memory map marker test double", () => {
  it("exposes the shared catalog and emits MapMarkers invalidations", async () => {
    const client = new MockReticulumNodeClient();
    const scopes: string[] = [];
    client.on("projectionInvalidated", (event) => scopes.push(event.scope));

    const symbols = await client.listMapMarkerSymbols();
    expect(symbols).toHaveLength(20);
    expect(symbols.find((symbol) => symbol.id === "vehicle")?.aliases).toContain("car");

    const { marker } = await client.createMapMarker({
      markerType: "medic",
      symbol: "medic",
      lat: 1,
      lon: 2,
    });
    expect(marker.category).toBe("medic");
    await client.renameMapMarker(marker.objectDestinationHash, "Aid post");
    await expect(client.listMapMarkers()).resolves.toMatchObject([{ name: "Aid post" }]);
    await expect(
      client.createMapMarker({ markerType: "x", symbol: "spaceship", lat: 0, lon: 0 }),
    ).rejects.toThrow("Unsupported marker symbol");
    await expect(client.deleteMapMarker(marker.objectDestinationHash)).resolves.toBe(true);
    expect(scopes.filter((scope) => scope === "MapMarkers")).toHaveLength(3);
  });
});
