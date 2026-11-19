import type { LibraryItem, LibraryItemKind, LibraryQuery } from "@reticulum/node-client";
import { defineStore } from "pinia";
import { ref, shallowRef } from "vue";

import { useNodeStore } from "./nodeStore";

export type LibrarySection = "device" | "rch" | "shared";
export type LibraryKindFilter = "all" | LibraryItemKind;

const SECTION_SOURCE = { device: "local", rch: "rch", shared: "peer" } as const;

/**
 * Projection cache for the Mission Library. Holds metadata and bounded
 * thumbnails only; the native runtime owns bytes, hashes and storage.
 */
export const useLibraryStore = defineStore("library", () => {
  const nodeStore = useNodeStore();
  const items = shallowRef<LibraryItem[]>([]);
  const missions = shallowRef<string[]>([]);
  const total = ref(0);
  const hasNext = ref(false);
  const page = ref(1);
  const section = ref<LibrarySection>("device");
  const kind = ref<LibraryKindFilter>("all");
  const mission = ref("");
  const search = ref("");
  const loading = ref(false);
  const busy = ref(false);
  const lastError = ref("");
  const hubStatus = ref<string | undefined>();
  const thumbnails = ref<Record<string, string>>({});
  let unsubscribe: (() => void) | null = null;

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  function query(): LibraryQuery {
    return {
      source: SECTION_SOURCE[section.value],
      kind: kind.value === "all" ? undefined : kind.value,
      mission_uid: mission.value || undefined,
      search: search.value.trim() || undefined,
      page: page.value,
      per_page: 20,
    };
  }

  async function refresh(): Promise<void> {
    loading.value = true;
    try {
      const client = await nodeStore.libraryClient();
      const listing = await client.listLibraryItems(query());
      items.value = listing.items;
      missions.value = listing.missions;
      total.value = listing.total;
      hasNext.value = listing.hasNext;
      hubStatus.value = listing.hubStatus;
      lastError.value = "";
      void loadThumbnails(listing.items);
    } catch (error: unknown) {
      lastError.value = errorMessage(error);
    } finally {
      loading.value = false;
    }
  }

  async function loadThumbnails(list: LibraryItem[]): Promise<void> {
    const client = await nodeStore.libraryClient();
    for (const item of list) {
      if (item.kind !== "image" || item.availability !== "local" || thumbnails.value[item.item_id]) {
        continue;
      }
      const url = await client.getLibraryThumbnail(item.item_id).catch(() => undefined);
      if (url) {
        thumbnails.value = { ...thumbnails.value, [item.item_id]: url };
      }
    }
  }

  function init(): Promise<void> {
    unsubscribe ??= nodeStore.onClientEvent("projectionInvalidated", (event) => {
      if (event.scope === "Library") {
        void refresh();
      }
    });
    return refresh();
  }

  async function run(operation: () => Promise<unknown>): Promise<void> {
    busy.value = true;
    try {
      await operation();
      lastError.value = "";
    } catch (error: unknown) {
      lastError.value = errorMessage(error);
    } finally {
      busy.value = false;
    }
  }

  return {
    items,
    missions,
    total,
    hasNext,
    page,
    section,
    kind,
    mission,
    search,
    loading,
    busy,
    lastError,
    thumbnails,
    hubStatus,
    init,
    refreshHub: () =>
      run(async () =>
        (await nodeStore.libraryClient()).refreshLibraryHub({
          kind: kind.value === "all" ? undefined : kind.value,
          mission_uid: mission.value || undefined,
          search: search.value.trim() || undefined,
          page: 1,
          per_page: 50,
        }),
      ),
    refresh,
    importFile: (pick: "all" | "image") =>
      run(async () => (await nodeStore.libraryClient()).importLibraryFile(pick)),
    open: (itemId: string) => run(async () => (await nodeStore.libraryClient()).openLibraryItem(itemId)),
    remove: (itemId: string) => run(async () => (await nodeStore.libraryClient()).deleteLibraryItem(itemId)),
    share: (itemId: string, destinationHex: string) =>
      run(async () => (await nodeStore.libraryClient()).shareLibraryItem(itemId, destinationHex)),
    download: (itemId: string) =>
      run(async () => (await nodeStore.libraryClient()).downloadLibraryItem(itemId)),
    setMissions: (itemId: string, uids: string[]) =>
      run(async () => (await nodeStore.libraryClient()).setLibraryItemMissions(itemId, uids)),
  };
});
