import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";

import { gotoApp, seedAppStorage } from "./support/app";

// Picture markers (r3akt-uas #19). Decoding, hash checks and the RCH request
// are native; this stubs the native projection the web client cannot produce.
const BLANK_MAP_STYLE = { version: 8, sources: {}, layers: [{ id: "bg", type: "background", paint: { "background-color": "#202a30" } }] };
const jpeg = (name: string): string =>
  `data:image/jpeg;base64,${readFileSync(`e2e/fixtures/${name}`).toString("base64")}`;
const THUMBNAIL = jpeg("picture-thumbnail.jpg");
const REDUCED = jpeg("picture-reduced.jpg");
const CENTER = { lat: 44.5, lon: -1.25 };

function pictureMarker(key: string, name: string, lon: number, thumbnailDataUrl?: string) {
  return {
    objectDestinationHash: key,
    originRch: "feedfeed",
    markerType: "picture",
    symbol: "picture",
    name,
    category: "picture",
    lat: CENTER.lat,
    lon,
    createdAtMs: Date.now(),
    updatedAtMs: Date.now(),
    picture: {
      messageId: key.padEnd(64, "0"),
      capturedAt: "2026-11-17T12:00:00Z",
      crop: "centre",
      thumbnailWidth: 160,
      thumbnailHeight: 120,
      thumbnailDataUrl,
    },
  };
}

async function openMapWithPictures(page: Page): Promise<void> {
  await page.route("https://tiles.openfreemap.org/styles/liberty*", (route) => route.fulfill({ json: BLANK_MAP_STYLE }));
  await seedAppStorage(page);
  await gotoApp(page, "/dashboard");
  await page.evaluate(async ({ markers, reduced }) => {
    const { useNodeStore } = await import("/src/stores/nodeStore.ts");
    // Full images come from RCH, which autonomous REM never asks.
    useNodeStore().settings.hub.mode = "SemiAutonomous";
    const client = await useNodeStore().mapMarkerClient();
    const w = window as any;
    w.pictureMarkers = markers;
    w.pictureRequests = [];
    client.listMapMarkers = async () => structuredClone(w.pictureMarkers);
    client.requestMapMarkerPicture = async (key: string) => {
      w.pictureRequests.push(key);
      w.pictureMarkers.find((m: any) => m.objectDestinationHash === key).picture.imageStatus = "requested";
    };
    client.getMapMarkerPictureImage = async () => reduced;
  }, {
    markers: [
      pictureMarker("a1", "Picture 1", CENTER.lon - 0.002, THUMBNAIL),
      pictureMarker("b2", "Picture 2", CENTER.lon + 0.002),
    ],
    reduced: REDUCED,
  });
  await page.getByRole("link", { name: "Map", exact: true }).click();
  await expect(page.locator(".map-container .maplibregl-canvas")).toBeVisible();
  await page.locator(".map-marker-layer").evaluate((el: any, center) => {
    el.__vueParentComponent.props.map.jumpTo({ center: [center.lon, center.lat], zoom: 15 });
  }, CENTER);
}

/** Simulates the native answer arriving: update the projection and refresh. */
async function answer(page: Page, key: string, imageStatus: string): Promise<void> {
  await page.evaluate(async ({ key, imageStatus }) => {
    const w = window as any;
    w.pictureMarkers.find((m: any) => m.objectDestinationHash === key).picture.imageStatus = imageStatus;
    const { useMapMarkersStore } = await import("/src/stores/mapMarkersStore.ts");
    await useMapMarkersStore().refresh();
  }, { key, imageStatus });
}

test("picture markers draw the thumbnail, fall back explicitly and load the full image on demand", async ({ page }, testInfo) => {
  await openMapWithPictures(page);

  const withThumb = page.getByRole("button", { name: "Picture marker: Picture 1" });
  await expect(withThumb.locator(".map-marker-picture-frame img")).toHaveAttribute("src", THUMBNAIL);
  await expect(withThumb.locator(".map-marker-picture-anchor")).toBeVisible();
  const frame = await withThumb.locator(".map-marker-picture-frame").boundingBox();
  expect(frame?.width).toBeLessThanOrEqual(70);

  const fallback = page.getByRole("button", { name: "Picture (no preview) marker: Picture 2" });
  await expect(fallback.locator(".map-marker-picture-frame.is-fallback svg")).toBeVisible();
  await expect(fallback.locator("img")).toHaveCount(0);
  await page.screenshot({ path: testInfo.outputPath("picture-markers.png") });

  // The add-marker picker does not offer picture markers.
  await page.getByRole("button", { name: "Add marker" }).click();
  const picker = page.getByRole("dialog", { name: "Choose a marker symbol" });
  await expect(picker.locator('[data-symbol="fire"]').first()).toBeVisible();
  await expect(picker.locator('[data-symbol="picture"]')).toHaveCount(0);
  await page.getByRole("button", { name: "Close symbol picker" }).click();

  await withThumb.click();
  const details = page.getByRole("dialog", { name: "Marker Picture 1" });
  const preview = details.getByTestId("marker-picture-preview");
  await expect(preview.getByRole("img", { name: "Picture thumbnail" })).toHaveAttribute("src", THUMBNAIL);
  await expect(preview).toContainText("where the camera was aimed, not a confirmed position");
  await page.screenshot({ path: testInfo.outputPath("picture-preview.png") });

  await preview.getByRole("button", { name: "Load full image" }).click();
  await expect.poll(() => page.evaluate(() => (window as any).pictureRequests)).toEqual(["a1"]);
  await answer(page, "a1", "requested");
  await expect(preview.getByRole("status")).toHaveText("Asking RCH for the full image…");
  await expect(preview.getByRole("button")).toHaveCount(0);

  await answer(page, "a1", "pending");
  await expect(preview.getByRole("status")).toContainText("has not received the full image");
  await answer(page, "a1", "unavailable");
  await expect(preview.getByRole("status")).toHaveText("The full image is not available on RCH.");
  await answer(page, "a1", "unauthorized");
  await expect(preview.getByRole("status")).toContainText("did not allow");
  await expect(preview.getByRole("button", { name: "Try loading full image again" })).toBeVisible();

  await answer(page, "a1", "ok");
  await expect(preview.getByRole("img", { name: "Full picture" })).toHaveAttribute("src", REDUCED);
  await expect(preview.getByRole("button")).toHaveCount(0);
  await page.screenshot({ path: testInfo.outputPath("picture-full.png") });

  await details.getByRole("button", { name: "Close marker details" }).click();
  await fallback.click();
  const fallbackPreview = page.getByRole("dialog", { name: "Marker Picture 2" }).getByTestId("marker-picture-preview");
  await expect(fallbackPreview.getByRole("img", { name: "No picture preview received" })).toBeVisible();
  await expect(fallbackPreview.getByRole("button", { name: "Load full image" })).toBeVisible();

  await page.evaluate(async () => {
    const { useNodeStore } = await import("/src/stores/nodeStore.ts");
    useNodeStore().settings.hub.mode = "Autonomous";
  });
  await expect(fallbackPreview.getByRole("button")).toHaveCount(0);
  await expect(fallbackPreview).toContainText("needs a link to RCH");
});
