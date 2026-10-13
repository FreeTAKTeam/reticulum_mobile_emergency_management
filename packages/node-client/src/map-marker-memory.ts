import type { MapMarkerCreateRequest, MapMarkerCreateResult, MapMarkerRecord, MapMarkerSymbolRecord } from "./contracts-map-markers";
import catalog from "./fixtures/map-marker-catalog.json";

/**
 * Test double for web/mock modes only. It is not a marker implementation:
 * native r3akt-map-core owns validation, normalization and idempotency.
 * The catalog fixture is pinned to the shared crate by a Rust test.
 */
export class InMemoryMapMarkers {
  private readonly markers = new Map<string, MapMarkerRecord>();
  private sequence = 0;

  constructor(private readonly onChange: (key: string, reason: string) => void) {}

  symbols(): MapMarkerSymbolRecord[] {
    const aliases = Object.entries(catalog.aliases as Record<string, string>);
    return catalog.symbols.map((symbol) => ({
      id: symbol.id,
      set: symbol.set,
      mdi: symbol.mdi ?? undefined,
      description: symbol.description,
      tak: symbol.tak ?? undefined,
      category: symbol.category,
      aliases: aliases.filter(([, id]) => id === symbol.id).map(([alias]) => alias),
    }));
  }

  list(): MapMarkerRecord[] {
    return [...this.markers.values()].map((marker) => ({ ...marker }));
  }

  create(request: MapMarkerCreateRequest): MapMarkerCreateResult {
    if (!catalog.symbols.some((symbol) => symbol.id === request.symbol)) {
      throw new Error("Unsupported marker symbol");
    }
    const now = Date.now();
    this.sequence += 1;
    const marker: MapMarkerRecord = {
      objectDestinationHash: `memory-marker-${this.sequence}`,
      originRch: "",
      markerType: request.markerType,
      symbol: request.symbol,
      name: request.name?.trim() || "Marker",
      category: request.category?.trim() || request.symbol,
      lat: request.lat,
      lon: request.lon,
      notes: request.notes,
      createdAtMs: now,
      updatedAtMs: now,
    };
    this.markers.set(marker.objectDestinationHash, marker);
    this.onChange(marker.objectDestinationHash, "marker-created");
    return { marker: { ...marker }, created: true };
  }

  update(key: string, patch: Partial<Pick<MapMarkerRecord, "name" | "lat" | "lon">>, reason: string): MapMarkerRecord {
    const marker = this.markers.get(key);
    if (!marker) {
      throw new Error("Marker not found");
    }
    Object.assign(marker, patch, { updatedAtMs: Date.now() });
    this.onChange(key, reason);
    return { ...marker };
  }

  delete(key: string): boolean {
    const deleted = this.markers.delete(key);
    if (deleted) {
      this.onChange(key, "marker-deleted");
    }
    return deleted;
  }
}
