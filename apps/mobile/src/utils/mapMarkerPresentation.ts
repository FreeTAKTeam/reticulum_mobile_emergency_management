import type { MapMarkerRecord, MapMarkerSymbolRecord } from "@reticulum/node-client";

export interface MapMarkerSymbolGroup {
  category: string;
  label: string;
  symbols: MapMarkerSymbolRecord[];
}

export const RECENT_SYMBOL_LIMIT = 4;

export function categoryLabel(category: string): string {
  if (category.toLowerCase() === "nato") {
    return "NATO";
  }
  return category
    .split(/[-_\s]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

/** Groups catalog symbols by their RCH category, keeping catalog order. */
export function groupSymbols(symbols: MapMarkerSymbolRecord[]): MapMarkerSymbolGroup[] {
  const groups = new Map<string, MapMarkerSymbolGroup>();
  for (const symbol of symbols) {
    let group = groups.get(symbol.category);
    if (!group) {
      group = { category: symbol.category, label: categoryLabel(symbol.category), symbols: [] };
      groups.set(symbol.category, group);
    }
    group.symbols.push(symbol);
  }
  return [...groups.values()];
}

/** Matches labels, IDs, categories and RCH aliases; empty query returns all. */
export function searchSymbols(
  symbols: MapMarkerSymbolRecord[],
  query: string,
): MapMarkerSymbolRecord[] {
  const needle = query.trim().toLowerCase();
  if (!needle) {
    return symbols;
  }
  return symbols.filter((symbol) =>
    [symbol.id, symbol.description, symbol.category, ...symbol.aliases]
      .some((value) => value.toLowerCase().includes(needle)),
  );
}

/** Most recent first, unique, limited, and only symbols still in the catalog. */
export function recentSymbols(
  recentIds: string[],
  symbols: MapMarkerSymbolRecord[],
  limit = RECENT_SYMBOL_LIMIT,
): MapMarkerSymbolRecord[] {
  const byId = new Map(symbols.map((symbol) => [symbol.id, symbol]));
  const seen = new Set<string>();
  const result: MapMarkerSymbolRecord[] = [];
  for (const id of recentIds) {
    const symbol = byId.get(id);
    if (symbol && !seen.has(id)) {
      seen.add(id);
      result.push(symbol);
    }
    if (result.length >= limit) {
      break;
    }
  }
  return result;
}

export function pushRecentSymbol(recentIds: string[], id: string, limit = RECENT_SYMBOL_LIMIT): string[] {
  return [id, ...recentIds.filter((existing) => existing !== id)].slice(0, limit);
}

/** Default name: the symbol label plus the next free number, e.g. "Fire 3". */
export function autoMarkerName(symbol: MapMarkerSymbolRecord, markers: MapMarkerRecord[]): string {
  const prefix = symbol.description;
  const pattern = new RegExp(`^${prefix.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")} (\\d+)$`);
  let highest = 0;
  for (const marker of markers) {
    const match = pattern.exec(marker.name);
    if (match) {
      highest = Math.max(highest, Number(match[1]));
    }
  }
  return `${prefix} ${highest + 1}`;
}

export function formatCoordinate(lat: number, lon: number): string {
  return `${lat.toFixed(5)}, ${lon.toFixed(5)}`;
}

export function markerAgeLabel(updatedAtMs: number, nowMs: number): string {
  const minutes = Math.max(0, Math.floor((nowMs - updatedAtMs) / 60_000));
  if (minutes < 1) {
    return "just now";
  }
  if (minutes < 60) {
    return `${minutes} min ago`;
  }
  const hours = Math.floor(minutes / 60);
  if (hours < 48) {
    return `${hours} h ago`;
  }
  return `${Math.floor(hours / 24)} d ago`;
}

export function markerOriginLabel(marker: MapMarkerRecord): string {
  return marker.originRch ? "RCH" : "Local";
}
