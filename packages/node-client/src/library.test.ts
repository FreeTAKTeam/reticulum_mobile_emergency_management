import { describe, expect, it } from "vitest";

import { toLibraryItem, toLibraryListing } from "./capacitor-library-client";
import { isLibraryClient } from "./contracts-library";

const item = {
  item_id: "local:abc",
  kind: "image",
  source: "local",
  availability: "local",
  name: "photo.jpg",
  size: 7,
  created_ts_ms: 1,
  updated_ts_ms: 1,
};

describe("library converters", () => {
  it("keeps valid items, drops malformed ones and defaults missions", () => {
    const listing = toLibraryListing({
      page: { items: [item, { name: 42 }, null], page: 2, per_page: 20, total: 21, has_next: false },
      missions: ["alpha", 7],
      hubStatus: "unknown_command",
    });
    expect(listing.hubStatus).toBe("unknown_command");
    expect(listing.items).toEqual([{ ...item, mission_uids: [] }]);
    expect([listing.page, listing.perPage, listing.total, listing.hasNext]).toEqual([2, 20, 21, false]);
    expect(listing.missions).toEqual(["alpha"]);
  });

  it("handles an empty or missing page", () => {
    expect(toLibraryListing({}).items).toEqual([]);
    expect(toLibraryItem(undefined)).toBeUndefined();
  });

  it("detects Library-capable clients", () => {
    expect(isLibraryClient({ listLibraryItems: () => undefined })).toBe(true);
    expect(isLibraryClient({})).toBe(false);
    expect(isLibraryClient(null)).toBe(false);
  });
});
