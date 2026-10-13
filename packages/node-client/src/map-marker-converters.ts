import type { MapMarkerRecord, MapMarkerSymbolRecord } from "./contracts-map-markers";

function text(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function optionalText(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

function finiteNumber(value: unknown): number {
  const number = typeof value === "number" ? value : Number(value);
  return Number.isFinite(number) ? number : 0;
}

export function toMapMarkerRecord(raw: Record<string, unknown>): MapMarkerRecord {
  return {
    objectDestinationHash: text(raw.objectDestinationHash),
    originRch: text(raw.originRch),
    markerType: text(raw.markerType),
    symbol: text(raw.symbol),
    name: text(raw.name),
    category: text(raw.category),
    lat: finiteNumber(raw.lat),
    lon: finiteNumber(raw.lon),
    notes: optionalText(raw.notes),
    createdAtMs: finiteNumber(raw.createdAtMs),
    updatedAtMs: finiteNumber(raw.updatedAtMs),
  };
}

export function toMapMarkerSymbolRecord(raw: Record<string, unknown>): MapMarkerSymbolRecord {
  return {
    id: text(raw.id),
    set: text(raw.set),
    mdi: optionalText(raw.mdi),
    description: text(raw.description),
    tak: optionalText(raw.tak),
    category: text(raw.category),
    aliases: Array.isArray(raw.aliases)
      ? raw.aliases.filter((alias): alias is string => typeof alias === "string")
      : [],
  };
}

export function toMapMarkerItems(result: { items?: unknown }): Record<string, unknown>[] {
  return Array.isArray(result.items)
    ? result.items.filter(
      (item): item is Record<string, unknown> => typeof item === "object" && item !== null,
    )
    : [];
}

export function toMapMarkerFromResult(result: Record<string, unknown>): MapMarkerRecord {
  const marker = result.marker;
  return toMapMarkerRecord(typeof marker === "object" && marker !== null
    ? marker as Record<string, unknown>
    : {});
}
