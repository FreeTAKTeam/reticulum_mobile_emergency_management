import { CapacitorMapMarkerClient } from "./capacitor-map-marker-client";
import type {
  LibraryClient,
  LibraryImportResult,
  LibraryItem,
  LibraryListing,
  LibraryQuery,
} from "./contracts-library";

export function toLibraryItem(value: unknown): LibraryItem | undefined {
  const item = value as Partial<LibraryItem> | null | undefined;
  return item && typeof item.item_id === "string" && typeof item.name === "string"
    ? ({ mission_uids: [], ...item } as LibraryItem)
    : undefined;
}

/** The runtime's `{ page, missions }` listing, with malformed entries dropped. */
export function toLibraryListing(raw: { page?: unknown; missions?: unknown[] }): LibraryListing {
  const page = (raw.page ?? {}) as Record<string, unknown>;
  const items = Array.isArray(page.items)
    ? page.items.map(toLibraryItem).filter((item): item is LibraryItem => item !== undefined)
    : [];
  return {
    items,
    page: Number(page.page ?? 1),
    perPage: Number(page.per_page ?? items.length),
    total: Number(page.total ?? items.length),
    hasNext: page.has_next === true,
    missions: Array.isArray(raw.missions)
      ? raw.missions.filter((mission): mission is string => typeof mission === "string")
      : [],
  };
}

/** Native Library calls; hashing, storage and integrity stay in the runtime. */
export abstract class CapacitorLibraryClient extends CapacitorMapMarkerClient implements LibraryClient {
  async listLibraryItems(query: LibraryQuery): Promise<LibraryListing> {
    await this.ready();
    return toLibraryListing(await this.plugin.listLibraryItems({ query }));
  }

  async importLibraryFile(kind: "all" | "image"): Promise<LibraryImportResult> {
    await this.ready();
    const raw = await this.plugin.importLibraryFile({ kind });
    if (raw.cancelled === true) {
      return { cancelled: true };
    }
    const item = toLibraryItem(raw.item);
    if (!item) {
      throw new Error("The runtime returned no Library item.");
    }
    return { cancelled: false, item };
  }

  async deleteLibraryItem(itemId: string): Promise<boolean> {
    await this.ready();
    return (await this.plugin.deleteLibraryItem({ itemId })).removed === true;
  }

  async setLibraryItemMissions(itemId: string, missionUids: string[]): Promise<LibraryItem | undefined> {
    await this.ready();
    return toLibraryItem((await this.plugin.setLibraryItemMissions({ itemId, missionUids })).item);
  }

  async openLibraryItem(itemId: string): Promise<void> {
    await this.ready();
    await this.plugin.openLibraryItem({ itemId });
  }

  async getLibraryThumbnail(itemId: string): Promise<string | undefined> {
    await this.ready();
    const { dataUrl } = await this.plugin.getLibraryThumbnail({ itemId });
    return typeof dataUrl === "string" && dataUrl.startsWith("data:image/") ? dataUrl : undefined;
  }

  async shareLibraryItem(itemId: string, destinationHex: string): Promise<void> {
    await this.ready();
    await this.plugin.shareLibraryItem({ itemId, destinationHex });
  }

  async downloadLibraryItem(itemId: string): Promise<void> {
    await this.ready();
    await this.plugin.downloadLibraryItem({ itemId });
  }
}
