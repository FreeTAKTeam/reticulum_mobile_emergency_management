/** Mission Library (REM #356): the shared `r3akt-library-core` item contract. */
export type LibraryItemKind = "file" | "image";
export type LibrarySource = "local" | "rch" | "peer";
export type LibraryAvailability = "remote" | "downloading" | "local" | "failed";

export interface LibraryRppReference {
  package_destination: string;
  file_destination: string;
  manifest_hash: string;
}

/** Metadata only; bytes stay in native storage. Field names match the Rust contract. */
export interface LibraryItem {
  item_id: string;
  legacy_file_id?: number;
  kind: LibraryItemKind;
  source: LibrarySource;
  availability: LibraryAvailability;
  name: string;
  media_type?: string;
  size: number;
  content_sha256?: string;
  rpp?: LibraryRppReference;
  publisher?: string;
  mission_uids: string[];
  team_uid?: string;
  created_ts_ms: number;
  updated_ts_ms: number;
}

export interface LibraryQuery {
  kind?: LibraryItemKind;
  source?: LibrarySource;
  mission_uid?: string;
  search?: string;
  page?: number;
  per_page?: number;
}

export interface LibraryListing {
  items: LibraryItem[];
  page: number;
  perPage: number;
  total: number;
  hasNext: boolean;
  /** Every mission UID present on the device, for the filter. */
  missions: string[];
  /** The hub's last Library answer: "ok", or why it refused (e.g. an older RCH). */
  hubStatus?: string;
}

export type LibraryImportResult = { cancelled: true } | { cancelled: false; item: LibraryItem };

export interface LibraryClient {
  listLibraryItems(query: LibraryQuery): Promise<LibraryListing>;
  /** Opens the platform document picker and imports the chosen file. */
  importLibraryFile(kind: "all" | "image"): Promise<LibraryImportResult>;
  deleteLibraryItem(itemId: string): Promise<boolean>;
  setLibraryItemMissions(itemId: string, missionUids: string[]): Promise<LibraryItem | undefined>;
  /** Hands a stored item to the platform viewer. */
  openLibraryItem(itemId: string): Promise<void>;
  /** A bounded JPEG thumbnail `data:` URL for stored images. */
  getLibraryThumbnail(itemId: string): Promise<string | undefined>;
  /** Offers a local item to one saved peer (RPP); progress arrives as Library updates. */
  shareLibraryItem(itemId: string, destinationHex: string): Promise<void>;
  /** Downloads an offered or hub-listed item; never started automatically. */
  downloadLibraryItem(itemId: string): Promise<void>;
  /** Asks the configured RCH for a page of its Library; answers update the projection. */
  refreshLibraryHub(query: LibraryQuery): Promise<void>;
}

export function isLibraryClient(client: unknown): client is LibraryClient {
  return typeof (client as Partial<LibraryClient> | null)?.listLibraryItems === "function";
}
